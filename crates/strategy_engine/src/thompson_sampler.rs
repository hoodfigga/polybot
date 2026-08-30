use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyArm {
    pub name: String,
    pub alpha: f64, // Wins + 1.0
    pub beta: f64,  // Losses + 1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualThompsonSampler {
    pub arms: Vec<StrategyArm>,
}

impl ContextualThompsonSampler {
    pub fn new() -> Self {
        Self {
            arms: vec![
                StrategyArm {
                    name: "DutchBookParity".to_string(),
                    alpha: 5.0,
                    beta: 1.0,
                },
                StrategyArm {
                    name: "AvellanedaStoikovMM".to_string(),
                    alpha: 4.0,
                    beta: 1.5,
                },
                StrategyArm {
                    name: "CrossVenueArb".to_string(),
                    alpha: 3.0,
                    beta: 1.0,
                },
                StrategyArm {
                    name: "NewsCatalystSniping".to_string(),
                    alpha: 3.0,
                    beta: 2.0,
                },
            ],
        }
    }

    /// Generates a sample from Beta(alpha, beta) using Johnk's generator or Gamma ratio
    fn sample_beta(&self, alpha: f64, beta: f64, rng: &mut impl Rng) -> f64 {
        // Fast approximation of Beta(a, b) via Gamma variate sampling or transform
        let a = alpha.max(0.1);
        let b = beta.max(0.1);

        let u1: f64 = rng.gen_range(1e-6..1.0);
        let u2: f64 = rng.gen_range(1e-6..1.0);

        // Approximate mean and variance parameterized beta sample
        let mean = a / (a + b);
        let var = (a * b) / ((a + b).powi(2) * (a + b + 1.0));
        let std_dev = var.sqrt();

        // Box-Muller normal approximation clamped to [0.01, 0.99]
        let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        let sample = mean + z0 * std_dev;
        sample.clamp(0.01, 0.99)
    }

    /// Samples Thompson capital weights dynamically across all active strategy arms
    pub fn sample_capital_weights(&self) -> Vec<(String, f64)> {
        let mut rng = rand::thread_rng();
        let mut samples = Vec::with_capacity(self.arms.len());
        let mut sum = 0.0;

        for arm in &self.arms {
            let sample = self.sample_beta(arm.alpha, arm.beta, &mut rng);
            samples.push((arm.name.clone(), sample));
            sum += sample;
        }

        if sum <= 1e-6 {
            let uniform = 1.0 / self.arms.len() as f64;
            return self.arms.iter().map(|a| (a.name.clone(), uniform)).collect();
        }

        let clamped: Vec<(String, f64)> = samples
            .into_iter()
            .map(|(name, s)| (name, (s / sum).max(0.05)))
            .collect();

        let clamped_sum: f64 = clamped.iter().map(|(_, w)| *w).sum();
        if clamped_sum > 1e-6 {
            clamped
                .into_iter()
                .map(|(name, w)| (name, w / clamped_sum))
                .collect()
        } else {
            clamped
        }
    }

    /// Updates Bayesian prior parameters based on trade outcome (win/loss feedback)
    pub fn record_trade_feedback(&mut self, strategy_name: &str, is_win: bool) {
        if let Some(arm) = self.arms.iter_mut().find(|a| a.name == strategy_name) {
            if is_win {
                arm.alpha = (arm.alpha + 1.0).min(500.0);
            } else {
                arm.beta = (arm.beta + 1.0).min(500.0);
            }
        }
    }
}

impl Default for ContextualThompsonSampler {
    fn default() -> Self {
        Self::new()
    }
}
