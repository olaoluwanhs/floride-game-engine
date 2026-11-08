/// A unique number used by the game to identify an entity.
///
/// An entity is an object in the game world, such as a player, enemy, or
/// pickup. This ID is owned by the game code. It is not a physics-engine ID,
/// so changing physics engines will not change the IDs used by the game.
///
/// Creating an `EntityId` only wraps a number; it does not check whether that
/// number is already in use. The game runtime must choose unique numbers.
///
/// The `derive` list asks Rust to provide common behaviors for this type:
/// `Debug` helps display it while debugging, `Clone` and `Copy` allow the small
/// ID value to be copied, `PartialEq` and `Eq` allow IDs to be compared, and
/// `Hash` allows IDs to be used as keys in hash-based collections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId {
    value: u64,
}

impl EntityId {
    /// Creates an entity ID from a number chosen by the game runtime.
    ///
    /// This function does not generate a new number or check for duplicates.
    /// The caller is responsible for providing an unused number.
    pub fn new(id_value: u64) -> Self {
        Self { value: id_value }
    }

    /// Returns the number stored in this entity ID.
    pub fn get(&self) -> u64 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::EntityId;

    #[test]
    fn entity_id_preserves_its_value() {
        let id = EntityId::new(42);

        assert_eq!(id.get(), 42);
        assert_eq!(id, EntityId::new(42));
    }
}
