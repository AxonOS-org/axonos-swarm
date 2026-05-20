//! Swarm scheduler — synchronised epoch start across N nodes.
//!
//! With Neural PTP keeping all nodes' clocks within 18 µs, the scheduler
//! releases each node's pipeline task group at the same global timestamp
//! (adjusted for local clock offset). This eliminates the distributed
//! co-availability problem: instead of synchronising delivery of outputs,
//! we synchronise the computation itself.
//!
//! ## Contract SC2 — Synchronised Start
//!
//! Each node starts its pipeline at local time corresponding to the global
//! swarm epoch t_start[k], with skew ≤ 2 × σ_sync ≤ 100 µs.
//!
//! ## Contract SC3 — Intent Co-availability
//!
//! Under SC1 (local WCRT ≤ 972 µs) and SC2 (skew ≤ 100 µs), all nodes'
//! intents are ready within ≤ 500 µs of each other.
//!
//! ## Reference
//!
//! AxonOS Article #36, § "Synchronised Scheduling: The Swarm Period"

use crate::neural_ptp::NeuralPtpKalman;

/// Swarm scheduler — computes local epoch start times using Neural PTP offset.
///
/// The swarm period is fixed at 4 ms (matching the single-node EDF period
/// from RFC-0004). All nodes share an epoch counter that increments in lockstep
/// via the PTP master clock.
#[derive(Debug)]
pub struct SwarmScheduler {
    /// Kalman-filtered clock synchroniser (SC0).
    ptp: NeuralPtpKalman,
    /// Monotonically increasing epoch counter shared across all nodes.
    epoch_index: u64,
    /// T_0: epoch origin in local µs, established at session start.
    epoch_origin_local_us: u64,
    /// Swarm period in µs. Default: 4000 µs (250 Hz epoch rate).
    swarm_period_us: u32,
}

impl SwarmScheduler {
    /// Swarm period: 4 ms, matching the single-node EDF period.
    pub const DEFAULT_PERIOD_US: u32 = 4_000;

    /// Maximum acceptable clock offset uncertainty for SC2 participation (µs, 3σ).
    pub const SYNC_QUALITY_THRESHOLD: f32 = 0.5;

    /// Construct a new scheduler with the default 4 ms period.
    ///
    /// `session_start_local_us` is the local timestamp (µs) of the first
    /// epoch origin, as established by the PTP master election.
    pub fn new(session_start_local_us: u64) -> Self {
        Self {
            ptp: NeuralPtpKalman::new(),
            epoch_index: 0,
            epoch_origin_local_us: session_start_local_us,
            swarm_period_us: Self::DEFAULT_PERIOD_US,
        }
    }

    /// Access the underlying Neural PTP filter for direct updates.
    pub fn ptp_mut(&mut self) -> &mut NeuralPtpKalman {
        &mut self.ptp
    }

    /// Current synchronisation quality (0.0 = poor, 1.0 = excellent).
    ///
    /// If below [`Self::SYNC_QUALITY_THRESHOLD`], the node should degrade
    /// to local-only operation (SC5) until PTP re-converges.
    pub fn sync_quality(&self) -> f32 {
        self.ptp.sync_quality()
    }

    /// Current epoch index.
    pub fn epoch_index(&self) -> u64 {
        self.epoch_index
    }

    /// Compute the **local µs timestamp** for the next swarm epoch start.
    ///
    /// This is the local time at which this node should release its pipeline
    /// task group in order to start in synchrony with all other nodes.
    ///
    /// The PTP offset correction converts global epoch time to local clock time:
    /// `t_local = t_global + θ_local`
    ///
    /// # Arguments
    ///
    /// * `local_now_us` — current local timestamp in µs (from on-chip timer).
    pub fn next_epoch_start_local_us(&mut self, local_now_us: u64) -> u64 {
        let offset_us = self.ptp.offset_us() as i64;
        let period = self.swarm_period_us as i64;
        let origin = self.epoch_origin_local_us as i64;

        // Global time ≈ local time − offset
        // Find next epoch boundary in global time
        let global_now = local_now_us as i64 - offset_us;
        let k_next = (global_now - origin) / period + 1;

        let global_next = origin + k_next * period;

        // Convert back to local time: t_local = t_global + offset
        (global_next + offset_us).max(local_now_us as i64) as u64
    }

    /// Advance the epoch counter to `k`.
    ///
    /// Call this at the start of each epoch to keep the internal counter
    /// in sync with the PTP master's epoch broadcast.
    pub fn set_epoch(&mut self, k: u64) {
        self.epoch_index = k;
    }

    /// Co-availability window (µs) for the current synchronisation state.
    ///
    /// This is the maximum spread between the fastest and slowest node's
    /// intent output time, given current clock precision:
    ///
    /// ```text
    /// Δ_ready ≤ WCRT_local_max − WCRT_local_min + Δ_start
    ///         ≤ (972 + 50) − 800 + 2 × σ_sync
    /// ```
    ///
    /// Returns `None` if sync quality is below threshold (SC5: local-only mode).
    pub fn co_availability_window_us(&self) -> Option<u64> {
        if self.ptp.sync_quality() < Self::SYNC_QUALITY_THRESHOLD {
            return None; // cannot guarantee SC3
        }
        let sigma_sync = self.ptp.offset_uncertainty_3sigma_us();
        let delta_start = 2.0 * sigma_sync;
        // Conservative: WCRT spread 222 µs (from Article #36)
        let wcrt_spread = 222.0_f64;
        Some(libm::ceil(wcrt_spread + delta_start) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_epoch_always_in_future() {
        let mut sched = SwarmScheduler::new(0);
        let now = 10_000_u64; // 10 ms
        let next = sched.next_epoch_start_local_us(now);
        assert!(
            next >= now,
            "next epoch must be in the future: next={next}, now={now}"
        );
    }

    #[test]
    fn next_epoch_within_one_period() {
        let mut sched = SwarmScheduler::new(0);
        let now = 10_000_u64;
        let next = sched.next_epoch_start_local_us(now);
        assert!(
            next - now <= SwarmScheduler::DEFAULT_PERIOD_US as u64,
            "next epoch too far: gap = {} µs",
            next - now
        );
    }

    #[test]
    fn co_availability_none_when_unsynced() {
        let sched = SwarmScheduler::new(0);
        // Fresh scheduler has high uncertainty → below sync threshold
        assert!(sched.co_availability_window_us().is_none());
    }

    #[test]
    fn co_availability_some_when_converged() {
        let mut sched = SwarmScheduler::new(0);
        // Converge the PTP filter with clean RF measurements
        for _ in 0..300 {
            sched.ptp_mut().predict(0.1);
            sched.ptp_mut().update(10.0);
        }
        let window = sched.co_availability_window_us();
        assert!(window.is_some());
        // Co-availability must be ≤ 500 µs per SC3
        assert!(window.unwrap() <= 500, "window = {} µs", window.unwrap());
    }
}
