//! Defines the [`GameAdapter`] trait.

use physics_api::PhysicsWorld;

/// The standard lifecycle interface that a game implementation provides to external hosts.
///
/// External hosts (such as command-line runners, browser WebAssembly drivers, or native
/// engine integrations) interact with games generically through this trait.
///
/// Rather than imposing a shared universal command list or snapshot structure across
/// all games, this trait uses **associated types**. Each game defines its own configuration,
/// inputs, snapshots, and events.
///
/// # Associated Types
/// - [`Physics`](GameAdapter::Physics): The physics world implementation used by the simulation.
/// - [`Config`](GameAdapter::Config): Setup parameters provided when creating the game instance.
/// - [`Input`](GameAdapter::Input): Game-specific commands or actions submitted by the host.
/// - [`Snapshot`](GameAdapter::Snapshot): State data exported for rendering, audio, or UI.
/// - [`Event`](GameAdapter::Event): Transient occurrences that happened during ticks (e.g. sounds, damage).
/// - [`Error`](GameAdapter::Error): Failures that can occur during creation, input processing, or stepping.
pub trait GameAdapter: Sized {
    /// The physics world implementation used by this game.
    type Physics: PhysicsWorld;

    /// Setup configuration passed when creating a new game session.
    type Config;

    /// Input commands or player actions accepted by this game.
    type Input;

    /// Read-only snapshot of the simulation state produced for display or rendering.
    type Snapshot;

    /// Simulation events produced during ticks (e.g., sound triggers, death alerts).
    type Event;

    /// Errors that can occur during initialization, input processing, or simulation ticks.
    type Error;

    /// Creates and initializes a new game session with the given configuration and physics world.
    fn create(config: Self::Config, physics: Self::Physics) -> Result<Self, Self::Error>;

    /// Submits a game-specific input command to be processed on or before the next tick.
    fn submit_input(&mut self, input: Self::Input) -> Result<(), Self::Error>;

    /// Advances the simulation by exactly one fixed-rate time step.
    fn tick(&mut self) -> Result<(), Self::Error>;

    /// Produces a snapshot of current simulation state for display or serialization.
    fn snapshot(&self) -> Self::Snapshot;

    /// Removes and returns all simulation events queued since the last call.
    fn drain_events(&mut self) -> Vec<Self::Event>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_types::Vec3;
    use physics_api::{
        CharacterDesc, CharacterMoveResult, DynamicBodyDesc, PhantomColliderDesc, PhantomOverlap,
        PhantomRayHit, PhysicsError, PhysicsHandle, PhysicsTransform, RayHit, RayQuery,
        StaticColliderDesc,
    };

    // Minimal physics world stub to satisfy the trait bound during tests.
    struct StubPhysics;

    impl PhysicsWorld for StubPhysics {
        fn create_static_collider(&mut self, _desc: &StaticColliderDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(1))
        }
        fn remove_static_collider(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn create_phantom_collider(&mut self, _desc: &PhantomColliderDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(2))
        }
        fn set_phantom_transform(&mut self, _handle: PhysicsHandle, _transform: &PhysicsTransform) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn remove_phantom_collider(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn phantom_overlaps(&self, _handle: PhysicsHandle) -> Result<Vec<PhantomOverlap>, PhysicsError> {
            Ok(Vec::new())
        }
        fn create_dynamic_body(&mut self, _desc: &DynamicBodyDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(3))
        }
        fn remove_dynamic_body(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn apply_impulse(&mut self, _handle: PhysicsHandle, _impulse: &Vec3) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn apply_impulse_at_point(&mut self, _handle: PhysicsHandle, _impulse: &Vec3, _point: &Vec3) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn create_character(&mut self, _desc: &CharacterDesc) -> Result<PhysicsHandle, PhysicsError> {
            Ok(PhysicsHandle::new(4))
        }
        fn remove_character(&mut self, _handle: PhysicsHandle) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn move_character(&mut self, _handle: PhysicsHandle, _displacement: &Vec3) -> Result<CharacterMoveResult, PhysicsError> {
            Ok(CharacterMoveResult {
                applied_displacement: Vec3::ZERO,
                is_grounded: true,
                collided: false,
            })
        }
        fn is_grounded(&self, _handle: PhysicsHandle) -> Result<bool, PhysicsError> {
            Ok(true)
        }
        fn raycast(&self, _query: &RayQuery) -> Result<Option<RayHit>, PhysicsError> {
            Ok(None)
        }
        fn raycast_phantoms(&self, _query: &RayQuery) -> Result<Vec<PhantomRayHit>, PhysicsError> {
            Ok(Vec::new())
        }
        fn step(&mut self, _dt: f32) -> Result<(), PhysicsError> {
            Ok(())
        }
        fn body_transform(&self, _handle: PhysicsHandle) -> Result<PhysicsTransform, PhysicsError> {
            Ok(PhysicsTransform::IDENTITY)
        }
    }

    // A minimal mock game to verify that a host can generically drive GameAdapter.
    struct MockGame {
        tick_count: u32,
        pending_inputs: Vec<String>,
        events: Vec<String>,
    }

    impl GameAdapter for MockGame {
        type Physics = StubPhysics;
        type Config = u32; // e.g. starting tick count
        type Input = String;
        type Snapshot = u32;
        type Event = String;
        type Error = String;

        fn create(config: Self::Config, _physics: Self::Physics) -> Result<Self, Self::Error> {
            Ok(Self {
                tick_count: config,
                pending_inputs: Vec::new(),
                events: Vec::new(),
            })
        }

        fn submit_input(&mut self, input: Self::Input) -> Result<(), Self::Error> {
            self.pending_inputs.push(input);
            Ok(())
        }

        fn tick(&mut self) -> Result<(), Self::Error> {
            self.tick_count += 1;
            for input in self.pending_inputs.drain(..) {
                self.events.push(format!("Handled: {}", input));
            }
            Ok(())
        }

        fn snapshot(&self) -> Self::Snapshot {
            self.tick_count
        }

        fn drain_events(&mut self) -> Vec<Self::Event> {
            std::mem::take(&mut self.events)
        }
    }

    struct HostTickResult<Snapshot, Event> {
        snapshot: Snapshot,
        events: Vec<Event>,
    }

    // Generic host function verifying that host code interacts strictly through GameAdapter.
    fn host_drive_tick<A: GameAdapter>(
        adapter: &mut A,
        input: A::Input,
    ) -> Result<HostTickResult<A::Snapshot, A::Event>, A::Error> {
        adapter.submit_input(input)?;
        adapter.tick()?;
        let snapshot = adapter.snapshot();
        let events = adapter.drain_events();
        Ok(HostTickResult { snapshot, events })
    }

    #[test]
    fn game_adapter_lifecycle_operates_as_expected() {
        let mut game = MockGame::create(10, StubPhysics).expect("Failed to create mock game");

        let result = host_drive_tick(&mut game, "Jump".to_string())
            .expect("Host driving tick failed");

        assert_eq!(result.snapshot, 11);
        assert_eq!(result.events, vec!["Handled: Jump".to_string()]);

        // Second drain should be empty since events were drained.
        assert!(game.drain_events().is_empty());
    }
}
