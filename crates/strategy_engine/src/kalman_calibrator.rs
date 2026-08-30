use serde::{Deserialize, Serialize};

/// Online Kalman Filter for Bayesian Real-Time Parameter Calibration (Layer 2 in Plan1.md)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineKalmanFilter {
    pub estimate: f64,          // Estimated state (e.g. dynamic gamma or true volatility)
    pub error_cov: f64,         // Estimation error covariance (P)
    pub process_noise: f64,      // Process noise covariance (Q)
    pub measurement_noise: f64,  // Measurement noise covariance (R)
}

impl OnlineKalmanFilter {
    pub fn new(initial_estimate: f64, process_noise_q: f64, measurement_noise_r: f64) -> Self {
        Self {
            estimate: initial_estimate,
            error_cov: 1.0,
            process_noise: process_noise_q,
            measurement_noise: measurement_noise_r,
        }
    }

    /// Recursively updates state estimate given a new noisy measurement
    pub fn update(&mut self, measurement: f64) -> f64 {
        if !measurement.is_finite() || measurement < 0.0 {
            return self.estimate;
        }

        // 1. Time update (predict prior)
        let p_prior = self.error_cov + self.process_noise;

        // 2. Compute Kalman Gain: K = P_prior / (P_prior + R)
        let denom = p_prior + self.measurement_noise;
        let kalman_gain = if denom.abs() <= 1e-9 {
            0.0
        } else {
            (p_prior / denom).clamp(0.0, 1.0)
        };

        // 3. Measurement update (correct estimate): x = x + K * (z - x)
        self.estimate += kalman_gain * (measurement - self.estimate);
        self.estimate = self.estimate.clamp(0.001, 1.0);

        // 4. Update error covariance: P = (1 - K) * P_prior
        self.error_cov = ((1.0 - kalman_gain) * p_prior).max(1e-6);

        self.estimate
    }
}

impl Default for OnlineKalmanFilter {
    fn default() -> Self {
        // Default calibrator for rolling volatility: Q=1e-4, R=1e-2
        Self::new(0.04, 0.0001, 0.01)
    }
}
