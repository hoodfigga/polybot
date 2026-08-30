use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Single Catalyst Evaluation & Resolution SFT Record (Layer 4 in Plan1.md)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalystResolutionPair {
    pub record_id: String,
    pub event_id: String,
    pub headline: String,
    pub prompt: String,
    pub predicted_prob: f64,
    pub recorded_at_unix: u64,
    pub resolved: bool,
    pub actual_outcome_prob: Option<f64>, // 1.0 (YES) or 0.0 (NO)
    pub prediction_error: Option<f64>,    // |predicted - actual|
}

/// SFT Dataset Formatted Training Example for LoRA Distillation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftTrainingExample {
    pub instruction: String,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionAccuracyLogger {
    pub records: Vec<CatalystResolutionPair>,
}

impl ResolutionAccuracyLogger {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Logs an initial catalyst assessment when news arrives
    pub fn log_prediction(
        &mut self,
        record_id: impl Into<String>,
        event_id: impl Into<String>,
        headline: impl Into<String>,
        prompt: impl Into<String>,
        predicted_prob: f64,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.records.push(CatalystResolutionPair {
            record_id: record_id.into(),
            event_id: event_id.into(),
            headline: headline.into(),
            prompt: prompt.into(),
            predicted_prob,
            recorded_at_unix: now,
            resolved: false,
            actual_outcome_prob: None,
            prediction_error: None,
        });
    }

    /// Records market settlement / UMA oracle resolution outcome
    pub fn record_market_resolution(
        &mut self,
        event_id: &str,
        actual_outcome_prob: f64,
    ) -> usize {
        let mut updated = 0;
        for record in self.records.iter_mut().filter(|r| r.event_id == event_id) {
            record.resolved = true;
            record.actual_outcome_prob = Some(actual_outcome_prob);
            record.prediction_error = Some((record.predicted_prob - actual_outcome_prob).abs());
            updated += 1;
        }
        updated
    }

    /// Exports all resolved prediction errors into formatted SFT training pairs
    pub fn export_sft_dataset(&self) -> Vec<SftTrainingExample> {
        self.records
            .iter()
            .filter(|r| r.resolved && r.actual_outcome_prob.is_some())
            .map(|r| {
                let actual = r.actual_outcome_prob.unwrap();
                SftTrainingExample {
                    instruction: "You are an institutional prediction market probabilistic reasoning model. Estimate the true probability of outcome YES based on the following breaking news headline:".to_string(),
                    input: r.headline.clone(),
                    output: format!("{{\"probability_shift\": {:.2}, \"confidence\": 0.95, \"resolved_ground_truth\": {:.1}}}", actual - r.predicted_prob, actual),
                }
            })
            .collect()
    }

    /// Computes Brier Score across all resolved predictions: (1/N) * sum((p_i - o_i)^2)
    pub fn compute_brier_score(&self) -> Option<f64> {
        let resolved_pairs: Vec<&CatalystResolutionPair> = self
            .records
            .iter()
            .filter(|r| r.resolved && r.actual_outcome_prob.is_some())
            .collect();

        if resolved_pairs.is_empty() {
            return None;
        }

        let sum_sq_err: f64 = resolved_pairs
            .iter()
            .map(|r| (r.predicted_prob - r.actual_outcome_prob.unwrap()).powi(2))
            .sum();

        Some(sum_sq_err / resolved_pairs.len() as f64)
    }
}

impl Default for ResolutionAccuracyLogger {
    fn default() -> Self {
        Self::new()
    }
}
