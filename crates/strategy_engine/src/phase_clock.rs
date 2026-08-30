use serde::{Deserialize, Serialize};

/// 5-Minute Rolling Window Phase (Plan3.md Module 2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketWindowPhase {
    WindowOpen,     // Sec 0-30: Settlement buffer (Entries locked)
    ActiveScalping, // Sec 30-240: Prime trading zone (Oracle lag & stale sniper enabled)
    ResolutionLean, // Sec 240-270: Late-stage directional zone (Half bet sizing)
    WindowGuard,    // Sec 270-300: Pre-resolution lockout (Hard entry lockout)
}

pub struct PhaseClock;

impl PhaseClock {
    /// Computes the current 5-minute market window phase and seconds elapsed in window
    #[inline(always)]
    pub fn get_current_phase(now_unix: u64) -> (MarketWindowPhase, u64) {
        let seconds_in_window = now_unix % 300;
        let phase = match seconds_in_window {
            0..=29 => MarketWindowPhase::WindowOpen,
            30..=239 => MarketWindowPhase::ActiveScalping,
            240..=269 => MarketWindowPhase::ResolutionLean,
            _ => MarketWindowPhase::WindowGuard,
        };
        (phase, seconds_in_window)
    }

    /// Determines if new position entry is permitted under the current window phase
    #[inline(always)]
    pub fn is_entry_allowed(phase: MarketWindowPhase) -> bool {
        matches!(
            phase,
            MarketWindowPhase::ActiveScalping | MarketWindowPhase::ResolutionLean
        )
    }

    /// Returns recommended position sizing multiplier based on phase
    #[inline(always)]
    pub fn size_multiplier(phase: MarketWindowPhase) -> f64 {
        match phase {
            MarketWindowPhase::ActiveScalping => 1.0,
            MarketWindowPhase::ResolutionLean => 0.5, // Half size in late-stage zone
            _ => 0.0,                                 // Zero size in open/guard zones
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_clock_partitioning() {
        // Base epoch window (0..300)
        let base_ts = 1780000000;
        let window_start = base_ts - (base_ts % 300);

        // 1. Sec 10 -> WindowOpen
        let (phase, sec) = PhaseClock::get_current_phase(window_start + 10);
        assert_eq!(phase, MarketWindowPhase::WindowOpen);
        assert_eq!(sec, 10);
        assert!(!PhaseClock::is_entry_allowed(phase));
        assert_eq!(PhaseClock::size_multiplier(phase), 0.0);

        // 2. Sec 120 -> ActiveScalping
        let (phase, sec) = PhaseClock::get_current_phase(window_start + 120);
        assert_eq!(phase, MarketWindowPhase::ActiveScalping);
        assert_eq!(sec, 120);
        assert!(PhaseClock::is_entry_allowed(phase));
        assert_eq!(PhaseClock::size_multiplier(phase), 1.0);

        // 3. Sec 250 -> ResolutionLean
        let (phase, sec) = PhaseClock::get_current_phase(window_start + 250);
        assert_eq!(phase, MarketWindowPhase::ResolutionLean);
        assert_eq!(sec, 250);
        assert!(PhaseClock::is_entry_allowed(phase));
        assert_eq!(PhaseClock::size_multiplier(phase), 0.5);

        // 4. Sec 285 -> WindowGuard
        let (phase, sec) = PhaseClock::get_current_phase(window_start + 285);
        assert_eq!(phase, MarketWindowPhase::WindowGuard);
        assert_eq!(sec, 285);
        assert!(!PhaseClock::is_entry_allowed(phase));
        assert_eq!(PhaseClock::size_multiplier(phase), 0.0);
    }
}
