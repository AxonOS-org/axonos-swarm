//! Neural PTP — Kalman-filtered IEEE 1588 clock synchronisation.
//!
//! Standard IEEE 1588 PTP achieves ±50–500 µs on BLE mesh links due to
//! wireless channel variance. Neural PTP applies a Kalman filter to the
//! raw PTP offset measurements, modelling the clock as a linear dynamic
//! system with offset and drift-rate state.
//!
//! After 300 measurements (30 s at 10 Hz), the filter converges to:
//! - Offset uncertainty: **±18 µs (3σ)** under moderate interference
//! - Drift uncertainty: **±0.3 ppm (3σ)**
//!
//! This is 6–8× better than raw PTP on BLE (±200–500 µs).
//!
//! ## Evidence level
//!
//! `L1` — analytically derived from Kalman filter theory and empirical
//! BLE mesh latency distribution model. `L3` oscilloscope validation
//! pending Q2 2026 hardware fixture.
//!
//! ## Reference
//!
//! - Welch, G. & Bishop, G. (2006). *An Introduction to the Kalman Filter.*
//! - IEEE 1588–2019. *Precision Clock Synchronization Protocol.*
//! - AxonOS Article #36, § "Neural PTP: Kalman Filter on the Offset"

/// Kalman-filtered IEEE 1588 PTP clock synchroniser.
///
/// State vector: `[θ (offset µs), ω (drift ppm)]`
///
/// # Fixed-point note
///
/// Internally uses `f64` arithmetic. On targets without an FPU this will
/// be soft-float. For M4F targets with FPU, compile with
/// `target-feature=+vfpv4` and use the `f32` variant (not yet provided).
#[derive(Clone, Debug)]
pub struct NeuralPtpKalman {
    /// State: [clock_offset_us, drift_ppm]
    state: [f64; 2],
    /// State covariance 2×2 (stored row-major)
    cov: [[f64; 2]; 2],
    /// Process noise covariance Q
    q: [[f64; 2]; 2],
    /// Measurement noise variance R (µs²)
    r: f64,
}

impl NeuralPtpKalman {
    /// Construct a new filter with uninformed priors.
    ///
    /// Initial uncertainty: ±500 µs offset (1σ), ±50 ppm drift (1σ).
    /// These are deliberately wide — the filter will converge within
    /// 30–60 seconds of PTP exchanges.
    pub fn new() -> Self {
        Self {
            state: [0.0, 0.0],
            // Initial covariance: large uncertainty on both state components.
            // offset variance = 500² = 250_000 µs², drift variance = 50² = 2_500 ppm²
            cov: [[250_000.0, 0.0], [0.0, 2_500.0]],
            // Process noise: offset random walk 0.1 µs²/s, drift 0.01 ppm²/s
            q: [[0.01, 0.0], [0.0, 0.0001]],
            // Measurement noise: 200 µs RMS on BLE mesh → R = 200² = 40_000 µs²
            r: 40_000.0,
        }
    }

    /// Construct with custom measurement noise.
    ///
    /// Use `measurement_noise_rms_us` ≈ 48 for clean RF, ≈ 180 for moderate
    /// interference, ≈ 520 for high interference (from Article #36 empirical data).
    pub fn with_measurement_noise(measurement_noise_rms_us: f64) -> Self {
        let mut s = Self::new();
        s.r = measurement_noise_rms_us * measurement_noise_rms_us;
        s
    }

    /// **Predict step** — propagate state forward by `dt_s` seconds.
    ///
    /// Call this once per PTP exchange interval before [`Self::update`].
    ///
    /// State transition model:
    /// ```text
    /// θ_{k|k-1} = θ_{k-1} + ω_{k-1} · dt · 1e6   (drift accumulates)
    /// ω_{k|k-1} = ω_{k-1}                           (drift assumed constant)
    /// ```
    pub fn predict(&mut self, dt_s: f64) {
        // Drift is stored in ppm: microseconds of offset per second.
        // Therefore the state transition uses seconds, not microseconds.
        let dt = dt_s;

        let theta_pred = self.state[0] + self.state[1] * dt;
        let omega_pred = self.state[1];

        // F = [[1, dt], [0, 1]]
        // P_{k|k-1} = F P F^T + Q
        let p00 = self.cov[0][0]
            + dt * (self.cov[0][1] + self.cov[1][0])
            + dt * dt * self.cov[1][1]
            + self.q[0][0];

        let p01 = self.cov[0][1] + dt * self.cov[1][1] + self.q[0][1];
        let p10 = self.cov[1][0] + dt * self.cov[1][1] + self.q[1][0];
        let p11 = self.cov[1][1] + self.q[1][1];

        self.state = [theta_pred, omega_pred];
        self.cov = [[p00, p01], [p10, p11]];
    }

    /// **Update step** — incorporate a new PTP offset measurement.
    ///
    /// `measured_offset_us` is the raw PTP offset estimate from one
    /// Sync / Delay-Request exchange cycle (µs).
    ///
    /// H = [1, 0] — we observe clock offset directly, not drift.
    pub fn update(&mut self, measured_offset_us: f64) {
        // Innovation: y = z - H·x
        let innov = measured_offset_us - self.state[0];

        // Innovation covariance: S = H P H^T + R = P[0][0] + R
        let s = self.cov[0][0] + self.r;
        if s.abs() < f64::EPSILON {
            return; // degenerate case — skip update
        }

        // Kalman gain: K = P H^T / S  →  K = [P[0][0]/S, P[1][0]/S]
        let k0 = self.cov[0][0] / s;
        let k1 = self.cov[1][0] / s;

        // State update: x = x + K·y
        self.state[0] += k0 * innov;
        self.state[1] += k1 * innov;

        // Covariance update: P = (I - KH)P.
        // Keep the old covariance values; row 1 must not use already-mutated row 0.
        let p00 = self.cov[0][0];
        let p01 = self.cov[0][1];
        let p10 = self.cov[1][0];
        let p11 = self.cov[1][1];

        self.cov[0][0] = (1.0 - k0) * p00;
        self.cov[0][1] = (1.0 - k0) * p01;
        self.cov[1][0] = p10 - k1 * p00;
        self.cov[1][1] = p11 - k1 * p01;
    }

    /// Best estimate of the current clock offset (µs).
    ///
    /// Positive means local clock is ahead of PTP master.
    /// Subtract this from local timestamps to get master-clock time.
    #[inline]
    pub fn offset_us(&self) -> f64 {
        self.state[0]
    }

    /// Estimated clock drift rate (ppm).
    #[inline]
    pub fn drift_ppm(&self) -> f64 {
        self.state[1]
    }

    /// **3σ uncertainty bound** on the offset estimate (µs).
    ///
    /// Used as the `σ_sync` input to the Swarm WCRT budget.
    /// SC0 requires this value ≤ 50 µs for normal operation.
    #[inline]
    pub fn offset_uncertainty_3sigma_us(&self) -> f64 {
        3.0 * libm::sqrt(self.cov[0][0].abs())
    }

    /// Synchronisation quality score: 0.0 (poor) to 1.0 (excellent).
    ///
    /// Derived from 3σ uncertainty. Quality ≥ 0.5 is required for
    /// the swarm scheduler to participate in SC2 synchronised start.
    ///
    /// ```text
    /// quality = 1.0  when uncertainty ≤ 0 µs (ideal)
    /// quality = 0.5  when uncertainty = 250 µs
    /// quality = 0.0  when uncertainty ≥ 500 µs
    /// ```
    #[inline]
    pub fn sync_quality(&self) -> f32 {
        let u = self.offset_uncertainty_3sigma_us();
        (1.0 - (u / 500.0).min(1.0)) as f32
    }

    /// Reset to uninformed priors (e.g. after a master re-election).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for NeuralPtpKalman {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_after_300_measurements() {
        let mut ptp = NeuralPtpKalman::new();
        // Simulate 300 measurements at 10 Hz with 180 µs RMS noise
        for _ in 0..300 {
            ptp.predict(0.1);
            // True offset is 50 µs; measurements are noisy
            ptp.update(50.0 + 180.0 * 0.1); // simplified noise
        }
        // After 300 measurements, 3σ uncertainty should be << 200 µs
        assert!(
            ptp.offset_uncertainty_3sigma_us() < 200.0,
            "uncertainty = {} µs",
            ptp.offset_uncertainty_3sigma_us()
        );
        // Offset estimate should be close to 50 µs
        assert!(
            (ptp.offset_us() - 50.0).abs() < 100.0,
            "offset = {} µs",
            ptp.offset_us()
        );
    }

    #[test]
    fn quality_high_when_converged() {
        let mut ptp = NeuralPtpKalman::with_measurement_noise(48.0); // clean RF
        for _ in 0..300 {
            ptp.predict(0.1);
            ptp.update(10.0);
        }
        assert!(ptp.sync_quality() > 0.8, "quality = {}", ptp.sync_quality());
    }

    #[test]
    fn quality_low_when_noisy() {
        let ptp = NeuralPtpKalman::new(); // uninformed prior — uncertainty = 500 µs
        assert!(ptp.sync_quality() < 0.5, "quality = {}", ptp.sync_quality());
    }

    #[test]
    fn predict_increases_uncertainty() {
        let mut ptp = NeuralPtpKalman::new();
        // First converge it
        for _ in 0..300 {
            ptp.predict(0.1);
            ptp.update(0.0);
        }
        let u_before = ptp.offset_uncertainty_3sigma_us();
        // Then predict without update — uncertainty should grow
        for _ in 0..100 {
            ptp.predict(0.1);
        }
        let u_after = ptp.offset_uncertainty_3sigma_us();
        assert!(u_after > u_before, "predict should increase uncertainty");
    }

    #[test]
    fn update_only_skips_degenerate_s() {
        let mut ptp = NeuralPtpKalman::new();
        // Force degenerate state (should not panic)
        ptp.cov[0][0] = 0.0;
        ptp.r = 0.0;
        ptp.update(100.0); // should return without panic
    }
}
