use std::sync::atomic::{AtomicU64, Ordering};

/// V-Shape Reversal Opposing Directional Cooldown Tracker (Plan3.md Module 3)
/// Prevents taking opposing positions during fast mean-reverting market wicks
pub struct DirectionalCooldownTracker {
    last_up_trade_time_unix: AtomicU64,
    last_down_trade_time_unix: AtomicU64,
    cooldown_duration_secs: u64,
}

impl DirectionalCooldownTracker {
    pub fn new(cooldown_duration_secs: u64) -> Self {
        Self {
            last_up_trade_time_unix: AtomicU64::new(0),
            last_down_trade_time_unix: AtomicU64::new(0),
            cooldown_duration_secs,
        }
    }

    /// Checks if a trade in `is_up` direction is permitted given prior opposing trades
    pub fn is_trade_direction_permitted(&self, is_up: bool, now_unix: u64) -> bool {
        if is_up {
            let last_down = self.last_down_trade_time_unix.load(Ordering::Relaxed);
            now_unix >= last_down + self.cooldown_duration_secs
        } else {
            let last_up = self.last_up_trade_time_unix.load(Ordering::Relaxed);
            now_unix >= last_up + self.cooldown_duration_secs
        }
    }

    /// Records an execution in `is_up` direction to arm the opposing lockout
    pub fn record_entry(&self, is_up: bool, now_unix: u64) {
        if is_up {
            self.last_up_trade_time_unix.store(now_unix, Ordering::Relaxed);
        } else {
            self.last_down_trade_time_unix
                .store(now_unix, Ordering::Relaxed);
        }
    }
}

impl Default for DirectionalCooldownTracker {
    fn default() -> Self {
        Self::new(25) // 25-second lockout as specified in Plan3.md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directional_cooldown_whipsaw_prevention() {
        let tracker = DirectionalCooldownTracker::new(25);
        let t0 = 1000;

        // Initially both directions permitted
        assert!(tracker.is_trade_direction_permitted(true, t0));
        assert!(tracker.is_trade_direction_permitted(false, t0));

        // Open UP trade at t0
        tracker.record_entry(true, t0);

        // At t0 + 10s: UP still allowed, but DOWN is locked out!
        assert!(tracker.is_trade_direction_permitted(true, t0 + 10));
        assert!(!tracker.is_trade_direction_permitted(false, t0 + 10));

        // At t0 + 25s: Lockout expired, DOWN permitted again
        assert!(tracker.is_trade_direction_permitted(false, t0 + 25));

        // Open DOWN trade at t0 + 30s
        tracker.record_entry(false, t0 + 30);

        // At t0 + 40s (10s later): UP is locked out
        assert!(!tracker.is_trade_direction_permitted(true, t0 + 40));
        assert!(tracker.is_trade_direction_permitted(false, t0 + 40));
    }
}
