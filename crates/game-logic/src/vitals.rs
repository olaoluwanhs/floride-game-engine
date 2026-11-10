//! Reusable health, damage, and elimination tracking primitives.
//!
//! Keeps vitals math (clamping, healing, non-finite guards, and one-time death
//! notifications) consistent across players, enemies, targets, and destructible props.

/// Result returned when applying damage to a [`HealthState`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageOutcome {
    /// Actual positive health deducted cleanly (0.0 if already dead or invalid input).
    pub damage_applied: f32,
    /// Remaining health after damage.
    pub remaining_hp: f32,
    /// `true` only on the exact hit that transitioned `is_alive` from `true` to `false`.
    pub eliminated_now: bool,
}

/// Engine-neutral vitals pool shared across entities in any game genre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealthState {
    /// Current health points (always within `0.0..=max_hp`).
    pub current_hp: f32,
    /// Maximum baseline health points.
    pub max_hp: f32,
    /// Whether this entity is still active/alive.
    pub is_alive: bool,
}

impl HealthState {
    /// Creates a new living entity at full health (clamping invalid/negative max HP to >= 1.0).
    pub fn new(max_hp: f32) -> Self {
        let safe_max = if max_hp.is_finite() && max_hp > 0.0 {
            max_hp
        } else {
            1.0
        };
        Self {
            current_hp: safe_max,
            max_hp: safe_max,
            is_alive: true,
        }
    }

    /// Deducts positive finite damage and reports whether this hit eliminated the entity.
    pub fn apply_damage(&mut self, amount: f32) -> DamageOutcome {
        if !self.is_alive || !amount.is_finite() || amount <= 0.0 {
            return DamageOutcome {
                damage_applied: 0.0,
                remaining_hp: self.current_hp,
                eliminated_now: false,
            };
        }

        let before = self.current_hp;
        self.current_hp = (self.current_hp - amount).max(0.0);
        let eliminated_now = self.current_hp <= 0.0;
        if eliminated_now {
            self.is_alive = false;
        }

        DamageOutcome {
            damage_applied: before - self.current_hp,
            remaining_hp: self.current_hp,
            eliminated_now,
        }
    }

    /// Restores up to `amount` health while alive, clamped at `max_hp`.
    pub fn heal(&mut self, amount: f32) -> f32 {
        if !self.is_alive || !amount.is_finite() || amount <= 0.0 {
            return 0.0;
        }
        let before = self.current_hp;
        self.current_hp = (self.current_hp + amount).min(self.max_hp);
        self.current_hp - before
    }

    /// Resets health back to `max_hp` and marks the entity alive (e.g. on respawn).
    pub fn restore_full(&mut self) {
        self.current_hp = self.max_hp;
        self.is_alive = true;
    }

    /// Normalized health fraction in `0.0..=1.0`.
    pub fn ratio(&self) -> f32 {
        if self.max_hp > 0.0 {
            (self.current_hp / self.max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HealthState;

    #[test]
    fn health_damage_elimination_and_restore_work() {
        let mut hp = HealthState::new(100.0);
        assert!(hp.is_alive);

        let hit1 = hp.apply_damage(40.0);
        assert_eq!(hit1.damage_applied, 40.0);
        assert_eq!(hit1.remaining_hp, 60.0);
        assert!(!hit1.eliminated_now);

        let healed = hp.heal(20.0);
        assert_eq!(healed, 20.0);
        assert_eq!(hp.current_hp, 80.0);

        let hit2 = hp.apply_damage(120.0);
        assert_eq!(hit2.damage_applied, 80.0);
        assert_eq!(hit2.remaining_hp, 0.0);
        assert!(hit2.eliminated_now);
        assert!(!hp.is_alive);

        // Further damage after death must not trigger elimination twice.
        let hit3 = hp.apply_damage(10.0);
        assert_eq!(hit3.damage_applied, 0.0);
        assert!(!hit3.eliminated_now);

        hp.restore_full();
        assert!(hp.is_alive);
        assert_eq!(hp.current_hp, 100.0);
    }
}
