// In Rust, declaring `mod` introduces another file in the same directory as a submodule.
// This is similar to importing a module in TypeScript, but in Rust, the root file
// (`main.rs` or `lib.rs`) explicitly declares the crate's module tree.
mod render;
mod scene;

use macroquad::prelude::{
    BLACK, KeyCode, clear_background, get_frame_time, is_key_down, is_key_pressed, next_frame,
    screen_width, set_default_camera,
};

use render::OrbitCamera;
use scene::Scene;

/// The physics simulation runs at a fixed frequency of 60 ticks per second (60 Hz).
///
/// Running physics at a fixed rate ensures consistent collisions, jumps, and velocities
/// regardless of whether the user has a 60 Hz, 144 Hz, or fluctuating monitor refresh rate.
const FIXED_TIME_STEP: f32 = 1.0 / 60.0;

/// Entry point for the visual demonstration application.
///
/// `#[macroquad::main(...)]` is an attribute macro that sets up the window and graphical
/// backend, wrapping our `async fn main()` so Macroquad can control the game loop.
#[macroquad::main("FPS Core Physics Showcase")]
async fn main() {
    // Initialize the scene. In Rust, functions that can fail return a `Result<T, E>`.
    // Instead of try/catch, we match on `Ok` (success) or `Err` (failure) to initialize
    // the scene state safely without crashing.
    let (mut scene, mut simulation_error) = match Scene::new() {
        Ok(scene) => (Some(scene), None),
        Err(error) => (None, Some(error)),
    };

    let mut camera = OrbitCamera::default();
    let mut accumulated_time = 0.0;
    let mut jump_queued = false;

    // Macroquad runs an asynchronous rendering loop. Each iteration processes one frame.
    loop {
        // `get_frame_time()` returns the elapsed wall-clock seconds since the previous frame.
        // We cap this delta to 0.1s (100ms) to prevent the "spiral of death" — a situation where
        // a sudden lag spike causes the physics loop below to run so many catch-up ticks
        // that it causes even more lag on the next frame.
        let frame_time = get_frame_time().min(0.1);
        camera.update(frame_time);
        clear_background(BLACK);

        if let Some(active_scene) = scene.as_mut() {
            // --- 1. ACCUMULATOR PATTERN FOR FIXED-TIMESTEP SIMULATION ---
            //
            // Rendering happens at variable frame rates (e.g. 60 FPS, 120 FPS), but physics
            // needs a constant time step. We accumulate elapsed frame time, and whenever we
            // have at least 1/60th of a second stored up, we advance the physics by one tick.
            accumulated_time += frame_time;

            // Discrete player inputs (like pressing Space to jump) can happen between frames.
            // We use bitwise OR (`|=`) so a jump press isn't forgotten if multiple frames pass
            // before the next fixed physics tick occurs.
            jump_queued |= is_key_pressed(KeyCode::Space);

            while accumulated_time >= FIXED_TIME_STEP {
                // Read WASD keys and rotate the direction to match the camera's viewing angle.
                let movement = read_movement_input(camera.yaw());

                // Advance the physics world by exactly one 60 Hz step.
                if let Err(error) = active_scene.tick(movement, jump_queued, FIXED_TIME_STEP) {
                    simulation_error = Some(error);
                    break;
                }

                // Reset the one-shot jump trigger and deduct the consumed time slice.
                jump_queued = false;
                accumulated_time -= FIXED_TIME_STEP;
            }

            // --- 2. RENDERING AND PRESENTATION ---
            //
            // If the simulation encountered an unrecoverable error, display it on screen.
            // Otherwise, fetch lightweight render snapshots from the scene and draw them.
            if let Some(error) = simulation_error.as_deref() {
                render::draw_error(error);
            } else {
                match active_scene.render_items() {
                    Ok(items) => {
                        render::draw_scene(&items, active_scene.ray_visuals(), &camera);
                    }
                    Err(error) => simulation_error = Some(error),
                }

                // Switch back to 2D UI coordinates to draw the text overlay (HUD).
                set_default_camera();
                render::draw_hud(active_scene.status_lines(), screen_width());
            }
        } else if let Some(error) = simulation_error.as_deref() {
            render::draw_error(error);
        }

        // Yield control back to Macroquad/OS until the next display frame is ready to draw.
        next_frame().await;
    }
}

/// Reads the WASD movement keys and converts them into a 3D world-space direction vector.
///
/// Moving in games should be intuitive: pressing 'W' should move the character towards
/// wherever the camera is pointing, not towards an arbitrary fixed world coordinate.
fn read_movement_input(camera_yaw: f32) -> game_types::Vec3 {
    let mut movement = game_types::Vec3::ZERO;

    // Collect 2D keyboard inputs along standard game axes:
    // A/D correspond to lateral (strafe) movement on X.
    // W/S correspond to forward/backward movement on Z.
    if is_key_down(KeyCode::A) {
        movement.x -= 1.0;
    }
    if is_key_down(KeyCode::D) {
        movement.x += 1.0;
    }
    if is_key_down(KeyCode::W) {
        movement.z += 1.0;
    }
    if is_key_down(KeyCode::S) {
        movement.z -= 1.0;
    }

    // When moving diagonally (e.g. W + D), the combined vector length is sqrt(1^2 + 1^2) ≈ 1.414.
    // Without normalization, diagonal movement would be 41% faster than moving in a straight line.
    let length_squared = movement.length_squared();
    if length_squared > 1.0 {
        let inverse_length = length_squared.sqrt().recip();
        movement = movement.scale(inverse_length);
    }

    // Convert screen-relative WASD input into world-space coordinates based on camera yaw.
    // `camera_yaw.sin_cos()` computes both sin(yaw) and cos(yaw) in a single CPU operation.
    let (sin_yaw, cos_yaw) = camera_yaw.sin_cos();
    let forward_x = -sin_yaw;
    let forward_z = -cos_yaw;
    let right_x = cos_yaw;
    let right_z = -sin_yaw;

    // Combine forward and right vectors scaled by the player's movement input.
    game_types::Vec3::new(
        movement.x * right_x + movement.z * forward_x,
        0.0,
        movement.x * right_z + movement.z * forward_z,
    )
}
