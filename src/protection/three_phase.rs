//! Three-phase overcurrent protection.
//!
//! Runs independent protection instances for phases A, B and C.  The consolidated
//! result is the **worst-case** across all three phases: `Trip` takes precedence
//! over `TripPending`, which takes precedence over `NoTrip`.
//!
//! Both [`ThreePhasePtoc`] and [`ThreePhasePioc`] share the same configuration
//! for all three phases (symmetric bay protection).  Per-phase configuration can
//! be achieved by constructing the inner instances directly and composing them.

use super::ptoc_sliding::PtocSlidingWindow;
use super::pioc::Pioc;
use super::traits::{ProtectionFunction, ProtectionResult};
use crate::config::{PtocConfig, PiocConfig};
use std::time::Duration;

/// Identifies which phase produced the worst-case result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    A,
    B,
    C,
}

/// Per-phase results plus a consolidated worst-case verdict.
#[derive(Debug, Clone)]
pub struct ThreePhaseResult {
    /// Result for phase A.
    pub phase_a: ProtectionResult,
    /// Result for phase B.
    pub phase_b: ProtectionResult,
    /// Result for phase C.
    pub phase_c: ProtectionResult,
    /// Worst-case result across all three phases.
    /// `Trip` > `TripPending` > `NoTrip` > `Disabled`.
    pub consolidated: ProtectionResult,
    /// Phase that produced the `consolidated` result (if it is `Trip` or `TripPending`).
    pub leading_phase: Option<Phase>,
}

/// Return the worse of two `ProtectionResult` values.
///
/// Severity order (highest first): `Trip` > `TripPending` > `NoTrip` > `Disabled`.
fn worst(a: ProtectionResult, b: ProtectionResult) -> ProtectionResult {
    match (&a, &b) {
        (ProtectionResult::Trip, _) | (_, ProtectionResult::Trip) => ProtectionResult::Trip,
        (ProtectionResult::TripPending(da), ProtectionResult::TripPending(db)) => {
            // Return the shorter remaining time (closer to tripping)
            ProtectionResult::TripPending(*da.min(db))
        }
        (ProtectionResult::TripPending(_), _) => a,
        (_, ProtectionResult::TripPending(_)) => b,
        (ProtectionResult::NoTrip, _) | (_, ProtectionResult::NoTrip) => ProtectionResult::NoTrip,
        _ => ProtectionResult::Disabled,
    }
}

/// Determine which phase has the leading (worst) result.
fn leading(
    ra: ProtectionResult,
    rb: ProtectionResult,
    rc: ProtectionResult,
    consolidated: &ProtectionResult,
) -> Option<Phase> {
    match consolidated {
        ProtectionResult::Trip | ProtectionResult::TripPending(_) => {
            if ra == *consolidated { Some(Phase::A) }
            else if rb == *consolidated { Some(Phase::B) }
            else { Some(Phase::C) }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// ThreePhasePtoc
// ---------------------------------------------------------------------------

/// Three-phase sliding-window PTOC.
///
/// All three phases share identical settings (`PtocConfig`).
/// Call [`ThreePhasePtoc::process`] once per SV sample with the three
/// primary current values (in Amperes).
pub struct ThreePhasePtoc {
    phase_a: PtocSlidingWindow,
    phase_b: PtocSlidingWindow,
    phase_c: PtocSlidingWindow,
}

impl ThreePhasePtoc {
    /// Create a new three-phase PTOC with identical settings on all phases.
    ///
    /// `samples_per_cycle` — the sliding-window length (e.g. 80 for 50 Hz @ 4 kSa/s).
    pub fn new(config: PtocConfig, samples_per_cycle: usize) -> Self {
        Self {
            phase_a: PtocSlidingWindow::new(config.clone(), samples_per_cycle),
            phase_b: PtocSlidingWindow::new(config.clone(), samples_per_cycle),
            phase_c: PtocSlidingWindow::new(config, samples_per_cycle),
        }
    }

    /// Evaluate all three phases against one set of instantaneous samples.
    ///
    /// `ia`, `ib`, `ic` — primary current in Amperes (instantaneous, not RMS).
    /// `timestamp` — microseconds since UNIX epoch.
    pub fn process(&mut self, ia: f64, ib: f64, ic: f64, timestamp: u64) -> ThreePhaseResult {
        let ra = self.phase_a.process_sample(ia, timestamp);
        let rb = self.phase_b.process_sample(ib, timestamp);
        let rc = self.phase_c.process_sample(ic, timestamp);
        let consolidated = worst(worst(ra.clone(), rb.clone()), rc.clone());
        let leading_phase = leading(ra.clone(), rb.clone(), rc.clone(), &consolidated);
        ThreePhaseResult { phase_a: ra, phase_b: rb, phase_c: rc, consolidated, leading_phase }
    }

    /// Reset all three phase instances.
    pub fn reset(&mut self) {
        self.phase_a.reset();
        self.phase_b.reset();
        self.phase_c.reset();
    }

    /// Update configuration on all phases; resets all instances.
    pub fn set_config(&mut self, config: PtocConfig, samples_per_cycle: usize) {
        self.phase_a = PtocSlidingWindow::new(config.clone(), samples_per_cycle);
        self.phase_b = PtocSlidingWindow::new(config.clone(), samples_per_cycle);
        self.phase_c = PtocSlidingWindow::new(config, samples_per_cycle);
    }
}

// ---------------------------------------------------------------------------
// ThreePhasePioc
// ---------------------------------------------------------------------------

/// Three-phase instantaneous overcurrent protection (PIOC).
///
/// All three phases share identical settings (`PiocConfig`).
/// Call [`ThreePhasePioc::process`] once per SV sample with the three
/// primary current values (in Amperes).
pub struct ThreePhasePioc {
    phase_a: Pioc,
    phase_b: Pioc,
    phase_c: Pioc,
}

impl ThreePhasePioc {
    /// Create a new three-phase PIOC with identical settings on all phases.
    pub fn new(config: PiocConfig) -> Self {
        Self {
            phase_a: Pioc::new(config.clone()),
            phase_b: Pioc::new(config.clone()),
            phase_c: Pioc::new(config),
        }
    }

    /// Evaluate all three phases against one set of instantaneous samples.
    pub fn process(&mut self, ia: f64, ib: f64, ic: f64, timestamp: u64) -> ThreePhaseResult {
        let ra = self.phase_a.process(ia, timestamp);
        let rb = self.phase_b.process(ib, timestamp);
        let rc = self.phase_c.process(ic, timestamp);
        let consolidated = worst(worst(ra.clone(), rb.clone()), rc.clone());
        let leading_phase = leading(ra.clone(), rb.clone(), rc.clone(), &consolidated);
        ThreePhaseResult { phase_a: ra, phase_b: rb, phase_c: rc, consolidated, leading_phase }
    }

    /// Reset all three phase instances.
    pub fn reset(&mut self) {
        self.phase_a.reset();
        self.phase_b.reset();
        self.phase_c.reset();
    }

    /// Update configuration on all phases; resets all instances.
    pub fn set_config(&mut self, config: PiocConfig) {
        self.phase_a = Pioc::new(config.clone());
        self.phase_b = Pioc::new(config.clone());
        self.phase_c = Pioc::new(config);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PtocConfig, PiocConfig};

    // ---- helpers -----------------------------------------------------------

    fn ptoc_config() -> PtocConfig {
        PtocConfig { iset: 100.0, tset: 100, enabled: true, ..PtocConfig::default() }
    }

    fn pioc_config() -> PiocConfig {
        PiocConfig { iset: 500.0, enabled: true, ..PiocConfig::default() }
    }

    // Advance n samples at `current` on all three phases
    fn fill_ptoc(ptoc: &mut ThreePhasePtoc, ia: f64, ib: f64, ic: f64, n: usize, t0: u64) -> ThreePhaseResult {
        let mut r = ThreePhaseResult {
            phase_a: ProtectionResult::NoTrip,
            phase_b: ProtectionResult::NoTrip,
            phase_c: ProtectionResult::NoTrip,
            consolidated: ProtectionResult::NoTrip,
            leading_phase: None,
        };
        for i in 0..n {
            r = ptoc.process(ia, ib, ic, t0 + i as u64 * 250);
        }
        r
    }

    // ---- ThreePhasePtoc tests -----------------------------------------------

    #[test]
    fn test_three_phase_ptoc_no_trip_all_below_pickup() {
        let mut ptoc = ThreePhasePtoc::new(ptoc_config(), 80);
        let r = fill_ptoc(&mut ptoc, 50.0, 50.0, 50.0, 80, 0);
        assert_eq!(r.consolidated, ProtectionResult::NoTrip);
        assert!(r.leading_phase.is_none());
    }

    #[test]
    fn test_three_phase_ptoc_trip_on_single_faulted_phase() {
        let mut ptoc = ThreePhasePtoc::new(ptoc_config(), 80);
        // Phase A overcurrent, B and C normal
        // Need > 100 ms (400 000 µs) at overcurrent to trip
        // Run enough samples at 4 kSa/s: 400 ms = 1600 samples
        let mut last = ThreePhaseResult {
            phase_a: ProtectionResult::NoTrip,
            phase_b: ProtectionResult::NoTrip,
            phase_c: ProtectionResult::NoTrip,
            consolidated: ProtectionResult::NoTrip,
            leading_phase: None,
        };
        for i in 0..1600u64 {
            last = ptoc.process(150.0, 50.0, 50.0, i * 250);
        }
        assert_eq!(last.consolidated, ProtectionResult::Trip);
        assert_eq!(last.leading_phase, Some(Phase::A));
        assert_eq!(last.phase_b, ProtectionResult::NoTrip);
        assert_eq!(last.phase_c, ProtectionResult::NoTrip);
    }

    #[test]
    fn test_three_phase_ptoc_trip_on_phase_c() {
        let mut ptoc = ThreePhasePtoc::new(ptoc_config(), 80);
        let mut last = ThreePhaseResult {
            phase_a: ProtectionResult::NoTrip,
            phase_b: ProtectionResult::NoTrip,
            phase_c: ProtectionResult::NoTrip,
            consolidated: ProtectionResult::NoTrip,
            leading_phase: None,
        };
        for i in 0..1600u64 {
            last = ptoc.process(50.0, 50.0, 150.0, i * 250);
        }
        assert_eq!(last.consolidated, ProtectionResult::Trip);
        assert_eq!(last.leading_phase, Some(Phase::C));
    }

    #[test]
    fn test_three_phase_ptoc_reset_clears_all_phases() {
        let mut ptoc = ThreePhasePtoc::new(ptoc_config(), 80);
        // Bring phase A into Pickup
        fill_ptoc(&mut ptoc, 150.0, 50.0, 50.0, 80, 0);
        ptoc.reset();
        // After reset, even high current should start fresh
        let r = ptoc.process(150.0, 50.0, 50.0, 100_000_000);
        // Only one sample — will be TripPending, not Trip
        assert!(matches!(r.phase_a, ProtectionResult::TripPending(_)));
        assert_ne!(r.consolidated, ProtectionResult::Trip);
    }

    // ---- ThreePhasePioc tests -----------------------------------------------

    #[test]
    fn test_three_phase_pioc_no_trip_all_below_pickup() {
        let mut pioc = ThreePhasePioc::new(pioc_config());
        let r = pioc.process(200.0, 200.0, 200.0, 0);
        assert_eq!(r.consolidated, ProtectionResult::NoTrip);
    }

    #[test]
    fn test_three_phase_pioc_trip_on_phase_b() {
        let mut pioc = ThreePhasePioc::new(pioc_config());
        let r = pioc.process(200.0, 600.0, 200.0, 0);
        assert_eq!(r.consolidated, ProtectionResult::Trip);
        assert_eq!(r.leading_phase, Some(Phase::B));
        assert_eq!(r.phase_a, ProtectionResult::NoTrip);
        assert_eq!(r.phase_c, ProtectionResult::NoTrip);
    }

    #[test]
    fn test_three_phase_pioc_trip_all_phases() {
        let mut pioc = ThreePhasePioc::new(pioc_config());
        let r = pioc.process(600.0, 600.0, 600.0, 0);
        assert_eq!(r.consolidated, ProtectionResult::Trip);
        assert_eq!(r.phase_a, ProtectionResult::Trip);
        assert_eq!(r.phase_b, ProtectionResult::Trip);
        assert_eq!(r.phase_c, ProtectionResult::Trip);
    }

    #[test]
    fn test_three_phase_pioc_reset_clears_all() {
        let mut pioc = ThreePhasePioc::new(pioc_config());
        pioc.process(600.0, 600.0, 600.0, 0);
        assert_eq!(pioc.phase_a.state(), super::super::traits::TripState::Trip);
        pioc.reset();
        assert_eq!(pioc.phase_a.state(), super::super::traits::TripState::Idle);
        assert_eq!(pioc.phase_b.state(), super::super::traits::TripState::Idle);
        assert_eq!(pioc.phase_c.state(), super::super::traits::TripState::Idle);
    }

    // ---- worst() helper tests -----------------------------------------------

    #[test]
    fn test_worst_trip_beats_pending() {
        let r = worst(
            ProtectionResult::Trip,
            ProtectionResult::TripPending(Duration::from_millis(50)),
        );
        assert_eq!(r, ProtectionResult::Trip);
    }

    #[test]
    fn test_worst_pending_shorter_wins() {
        let r = worst(
            ProtectionResult::TripPending(Duration::from_millis(80)),
            ProtectionResult::TripPending(Duration::from_millis(30)),
        );
        assert_eq!(r, ProtectionResult::TripPending(Duration::from_millis(30)));
    }

    #[test]
    fn test_worst_pending_beats_no_trip() {
        let r = worst(
            ProtectionResult::NoTrip,
            ProtectionResult::TripPending(Duration::from_millis(10)),
        );
        assert!(matches!(r, ProtectionResult::TripPending(_)));
    }
}
