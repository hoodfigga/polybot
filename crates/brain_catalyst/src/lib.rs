pub mod resolution_logger;
pub use resolution_logger::{CatalystResolutionPair, ResolutionAccuracyLogger, SftTrainingExample};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalystAssessment {
    pub market_identifier: String,
    pub headline: String,
    pub estimated_probability_shift: f64,
    pub confidence: f64,
    pub recommended_action: String, // "BUY_YES", "BUY_NO", "NO_ACTION"
    pub rationale: String,
}

pub struct CatalystNewsEngine {
    http_client: Client,
    ollama_endpoint: String,
    model_name: String,
}

impl CatalystNewsEngine {
    pub fn new(ollama_endpoint: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            http_client: Client::builder()
                .timeout(Duration::from_millis(3500))
                .build()
                .unwrap_or_default(),
            ollama_endpoint: ollama_endpoint.into().trim_end_matches('/').to_string(),
            model_name: model_name.into(),
        }
    }

    /// Evaluates a breaking news headline or OddsQ story against an active prediction market
    pub async fn evaluate_breaking_news(
        &self,
        market_question: &str,
        current_yes_price: f64,
        headline: &str,
    ) -> Option<CatalystAssessment> {
        let system_prompt = r#"You are a high-speed prediction market trading catalyst evaluator.
Analyze the breaking news against the market question.
Respond ONLY with a valid raw JSON object conforming exactly to this structure:
{
  "estimated_probability": <float 0.0 to 1.0>,
  "confidence": <float 0.0 to 1.0>,
  "rationale": "<concise 1-sentence explanation>"
}"#;

        let user_prompt = format!(
            "Market Question: \"{}\"\nCurrent YES Price: {:.2}\nBreaking Headline: \"{}\"\nAssess the probability impact:",
            market_question, current_yes_price, headline
        );

        let payload = serde_json::json!({
            "model": self.model_name,
            "system": system_prompt,
            "prompt": user_prompt,
            "stream": false,
            "format": "json"
        });

        let url = format!("{}/api/generate", self.ollama_endpoint);
        let resp = match self.http_client.post(&url).json(&payload).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Ollama HTTP request failed: {}", e);
                return None;
            }
        };

        if !resp.status().is_success() {
            tracing::warn!("Ollama returned non-success status: {}", resp.status());
            return None;
        }

        let resp_json = match resp.json::<serde_json::Value>().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to deserialize Ollama response: {}", e);
                return None;
            }
        };

        let raw_response = resp_json["response"].as_str().unwrap_or("");
        let parsed = match Self::parse_robust_json(raw_response) {
            Some(p) => p,
            None => {
                tracing::warn!("Failed to parse robust JSON from LLM: {}", raw_response);
                return None;
            }
        };

        let new_prob = extract_f64(&parsed["estimated_probability"]).unwrap_or(current_yes_price);
        let confidence = extract_f64(&parsed["confidence"]).unwrap_or(0.50);
        let rationale = parsed["rationale"].as_str().unwrap_or("").to_string();

        let shift = (new_prob - current_yes_price).clamp(-1.0, 1.0);
        let recommended_action = if shift > 0.05 && confidence > 0.65 {
            "BUY_YES".to_string()
        } else if shift < -0.05 && confidence > 0.65 {
            "BUY_NO".to_string()
        } else {
            "NO_ACTION".to_string()
        };

        Some(CatalystAssessment {
            market_identifier: market_question.to_string(),
            headline: headline.to_string(),
            estimated_probability_shift: shift,
            confidence: confidence.clamp(0.0, 1.0),
            recommended_action,
            rationale,
        })
    }

    /// Robust JSON extractor capable of stripping markdown code fences (```json ... ```)
    /// and extracting balanced JSON dictionaries from noisy LLM outputs
    pub fn parse_robust_json(raw: &str) -> Option<serde_json::Value> {
        let trimmed = raw.trim();

        // Direct attempt
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            return Some(v);
        }

        // Substring extraction within first { and last }
        if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
            if start < end {
                let slice = &trimmed[start..=end];
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(slice) {
                    return Some(v);
                }
            }
        }

        None
    }
}

fn extract_f64(val: &serde_json::Value) -> Option<f64> {
    val.as_f64()
        .or_else(|| val.as_str().and_then(|s| s.parse::<f64>().ok()))
        .or_else(|| val.as_i64().map(|i| i as f64))
        .or_else(|| val.as_u64().map(|u| u as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_robust_json_parser_markdown_wrapper() {
        let messy_llm_response = r#"
```json
{
  "estimated_probability": "0.85",
  "confidence": 1,
  "rationale": "High certainty fed rate cut signal."
}
```
"#;

        let parsed = CatalystNewsEngine::parse_robust_json(messy_llm_response)
            .expect("Must parse robust JSON");
        assert_eq!(extract_f64(&parsed["estimated_probability"]), Some(0.85));
        assert_eq!(extract_f64(&parsed["confidence"]), Some(1.0));
    }

    #[test]
    fn test_catalyst_assessment_schema() {
        let assessment = CatalystAssessment {
            market_identifier: "Will Federal Reserve Cut Rates in September?".to_string(),
            headline: "Fed Chair signals upcoming interest rate reduction in speech".to_string(),
            estimated_probability_shift: 0.22,
            confidence: 0.91,
            recommended_action: "BUY_YES".to_string(),
            rationale: "Chair explicitly guided towards monetary easing.".to_string(),
        };

        assert_eq!(assessment.recommended_action, "BUY_YES");
        assert!(assessment.estimated_probability_shift > 0.15);
    }

    #[test]
    fn test_resolution_accuracy_logger_and_sft() {
        let mut logger = ResolutionAccuracyLogger::new();

        // 1. Log predictions
        logger.log_prediction(
            "rec_1",
            "event_fed_sept",
            "Fed Chair remarks signal rate cuts",
            "Assess probability impact",
            0.85,
        );

        logger.log_prediction(
            "rec_2",
            "event_cpi_dec",
            "CPI inflation heats up unexpectedly",
            "Assess probability impact",
            0.30,
        );

        assert_eq!(logger.records.len(), 2);

        // 2. Record market resolutions (Event 1 settled YES = 1.0, Event 2 settled NO = 0.0)
        let updated1 = logger.record_market_resolution("event_fed_sept", 1.0);
        let updated2 = logger.record_market_resolution("event_cpi_dec", 0.0);
        assert_eq!(updated1, 1);
        assert_eq!(updated2, 1);

        // 3. Compute Brier score: ((0.85 - 1.0)^2 + (0.30 - 0.0)^2) / 2 = (0.0225 + 0.09) / 2 = 0.05625
        let brier = logger.compute_brier_score().expect("Brier score must be calculated");
        assert!((brier - 0.05625).abs() < 1e-4);

        // 4. Export SFT Dataset
        let sft_dataset = logger.export_sft_dataset();
        assert_eq!(sft_dataset.len(), 2);
        assert!(sft_dataset[0].output.contains("resolved_ground_truth"));
    }
}
