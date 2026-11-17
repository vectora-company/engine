//! Typed, non-streaming decision requests for providers such as llama-server.

use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

/// A persistable provider-neutral decision request without media bytes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    /// Optional target/model selector. It is local metadata for llama-server
    /// discovery and may be omitted when a SystemOne endpoint routes requests.
    #[serde(default, skip_serializing)]
    pub model: Option<String>,
    pub state: serde_json::Value,
    pub questions: BTreeMap<String, DecisionQuestion>,
}

/// Request-only media envelope, deliberately separate from `DecisionRequest`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EphemeralDecisionRequest {
    pub request: DecisionRequest,
    #[serde(default, alias = "images", skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

/// One independent typed question. `criteria` has a shape determined by kind:
/// choice object, score array, or an optional true/false map for noul.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DecisionQuestion {
    #[serde(rename = "type")]
    pub kind: DecisionQuestionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criteria: Option<serde_json::Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionQuestionKind {
    Choice,
    Score,
    Noul,
}

/// Probabilities and provider-specific answer fields are preserved verbatim.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DecisionResponse {
    pub answers: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<DecisionUsage>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DecisionUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
}

impl DecisionRequest {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self
            .model
            .as_deref()
            .is_some_and(|model| model.trim().is_empty())
        {
            return Err("model must not be empty when provided");
        }
        if self.questions.is_empty() {
            return Err("at least one question is required");
        }
        fn contains_state_media(value: &serde_json::Value) -> bool {
            match value {
                serde_json::Value::Array(items) => items.iter().any(contains_state_media),
                serde_json::Value::Object(items) => {
                    items.keys().any(|key| {
                        matches!(key.as_str(), "image_url" | "input_audio" | "input_video")
                    }) || items.values().any(contains_state_media)
                }
                _ => false,
            }
        }
        if contains_state_media(&self.state) {
            return Err("decision media must use the ephemeral files field");
        }
        for (id, question) in &self.questions {
            if id.trim().is_empty() {
                return Err("question identifiers must not be empty");
            }
            let criteria = question.criteria.as_ref();
            match question.kind {
                DecisionQuestionKind::Choice
                    if !criteria.is_some_and(serde_json::Value::is_object)
                        || criteria
                            .and_then(serde_json::Value::as_object)
                            .is_some_and(|v| v.is_empty()) =>
                {
                    return Err("choice questions require a non-empty criteria object");
                }
                DecisionQuestionKind::Score
                    if !criteria
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|v| (1..=255).contains(&v.len())) =>
                {
                    return Err("score questions require 1 to 255 criteria levels");
                }
                DecisionQuestionKind::Noul if criteria.is_some_and(|v| !v.is_object()) => {
                    return Err("noul criteria must be an object when provided");
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl EphemeralDecisionRequest {
    pub(crate) fn validate_with_limits(
        &self,
        max_media_bytes: u64,
        max_media_parts: Option<usize>,
    ) -> Result<(), &'static str> {
        self.request.validate()?;
        let mut total = 0u64;
        let mut parts = 0usize;
        for file in &self.files {
            let Some((metadata, encoded)) = file.split_once(',') else {
                return Err("decision files must be data URLs");
            };
            if !metadata.starts_with("data:image/") && !metadata.starts_with("data:audio/") {
                return Err("decision files support image or audio data URLs only");
            }
            if !metadata.ends_with(";base64") {
                return Err("decision file data URLs must use base64");
            }
            if max_media_parts.is_some_and(|limit| parts >= limit) {
                return Err("decision media exceeds the configured asset limit");
            }
            let remaining_bytes = max_media_bytes.saturating_sub(total);
            let max_encoded_bytes = ((remaining_bytes.saturating_add(2)) / 3).saturating_mul(4);
            if u64::try_from(encoded.len()).unwrap_or(u64::MAX) > max_encoded_bytes {
                return Err("decision media exceeds the configured limit");
            }
            let decoded = STANDARD
                .decode(encoded)
                .map_err(|_| "decision file contains invalid base64")?;
            total = total
                .checked_add(decoded.len() as u64)
                .ok_or("decision media exceeds the configured limit")?;
            if total > max_media_bytes {
                return Err("decision media exceeds the configured limit");
            }
            parts = parts
                .checked_add(1)
                .ok_or("decision media exceeds the configured asset limit")?;
            if max_media_parts.is_some_and(|limit| parts > limit) {
                return Err("decision media exceeds the configured asset limit");
            }
        }
        Ok(())
    }
}

pub(crate) fn answer_matches(question: &DecisionQuestion, answer: &serde_json::Value) -> bool {
    fn probability(value: Option<&serde_json::Value>) -> Option<f64> {
        value
            .and_then(serde_json::Value::as_f64)
            .filter(|number| number.is_finite() && (0.0..=1.0).contains(number))
    }
    fn distribution(values: impl Iterator<Item = f64>) -> bool {
        let values: Vec<_> = values.collect();
        !values.is_empty() && (values.iter().sum::<f64>() - 1.0).abs() <= 0.001
    }
    let Some(answer) = answer.as_object() else {
        return false;
    };
    match question.kind {
        DecisionQuestionKind::Choice => {
            let choice = answer.get("choice").and_then(serde_json::Value::as_str);
            let probabilities = answer
                .get("probabilities")
                .and_then(serde_json::Value::as_object);
            let criteria = question
                .criteria
                .as_ref()
                .and_then(serde_json::Value::as_object);
            choice.is_some_and(|choice| criteria.is_some_and(|items| items.contains_key(choice)))
                && answer
                    .get("confidence")
                    .and_then(serde_json::Value::as_f64)
                    .is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value))
                && probabilities.is_some_and(|items| {
                    items.len() == criteria.map_or(0, serde_json::Map::len)
                        && items
                            .keys()
                            .all(|key| criteria.is_some_and(|criteria| criteria.contains_key(key)))
                        && distribution(items.values().filter_map(|value| probability(Some(value))))
                        && items.len()
                            == items
                                .values()
                                .filter(|value| probability(Some(value)).is_some())
                                .count()
                })
        }
        DecisionQuestionKind::Score => {
            let levels = question
                .criteria
                .as_ref()
                .and_then(serde_json::Value::as_array);
            let probabilities = answer
                .get("probabilities")
                .and_then(serde_json::Value::as_array);
            let score = answer.get("score").and_then(serde_json::Value::as_f64);
            levels
                .zip(probabilities)
                .zip(score)
                .is_some_and(|((levels, probabilities), score)| {
                    score.is_finite()
                        && score >= 0.0
                        && score <= levels.len().saturating_sub(1) as f64
                        && probabilities.len() == levels.len()
                        && answer
                            .get("legend")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|legend| legend.len() == levels.len())
                        && answer
                            .get("confidence")
                            .and_then(serde_json::Value::as_f64)
                            .is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value))
                        && distribution(
                            probabilities
                                .iter()
                                .filter_map(|value| probability(Some(value))),
                        )
                        && probabilities
                            .iter()
                            .all(|value| probability(Some(value)).is_some())
                })
        }
        DecisionQuestionKind::Noul => probability(answer.get("noul")).is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(file: &str) -> EphemeralDecisionRequest {
        EphemeralDecisionRequest {
            request: DecisionRequest {
                model: Some("decision".into()),
                state: serde_json::json!({"case": "example"}),
                questions: BTreeMap::from([(
                    "route".into(),
                    DecisionQuestion {
                        kind: DecisionQuestionKind::Choice,
                        instructions: None,
                        criteria: Some(serde_json::json!({"billing": null, "other": null})),
                    },
                )]),
            },
            files: vec![file.into()],
        }
    }

    #[test]
    fn media_requires_base64_data_url_and_obeys_decoded_byte_limit() {
        assert!(
            request("data:image/png;base64,aGVsbG8=")
                .validate_with_limits(5, None)
                .is_ok()
        );
        assert_eq!(
            request("https://example.invalid/image.png").validate_with_limits(100, None),
            Err("decision files must be data URLs")
        );
        assert_eq!(
            request("data:image/png;base64,***").validate_with_limits(100, None),
            Err("decision file contains invalid base64")
        );
        assert_eq!(
            request("data:image/png;base64,aGVsbG8=").validate_with_limits(4, None),
            Err("decision media exceeds the configured limit")
        );
    }

    #[test]
    fn decision_files_share_aggregate_byte_and_asset_limits() {
        let mut request = request("data:image/png;base64,aGVsbG8=");
        request.files.push("data:audio/wav;base64,aGVsbG8=".into());
        assert_eq!(
            request.validate_with_limits(9, None),
            Err("decision media exceeds the configured limit")
        );
        assert_eq!(
            request.validate_with_limits(20, Some(1)),
            Err("decision media exceeds the configured asset limit")
        );
        assert!(request.validate_with_limits(10, Some(2)).is_ok());
    }

    #[test]
    fn images_is_an_input_alias_for_files() {
        let request: EphemeralDecisionRequest = serde_json::from_value(serde_json::json!({
            "request": {"state": null, "questions": {"visible": {"type": "noul"}}},
            "images": ["data:image/png;base64,aGVsbG8="]
        }))
        .unwrap();
        assert_eq!(request.request.model, None);
        assert_eq!(request.files, ["data:image/png;base64,aGVsbG8="]);
        let envelope = serde_json::to_value(request).unwrap();
        assert_eq!(envelope["files"][0], "data:image/png;base64,aGVsbG8=");
        assert!(envelope.get("images").is_none());
    }

    #[test]
    fn validates_typed_answer_shape_and_probability_distribution() {
        let question = DecisionQuestion {
            kind: DecisionQuestionKind::Choice,
            instructions: None,
            criteria: Some(serde_json::json!({"yes": null, "no": null})),
        };
        assert!(answer_matches(
            &question,
            &serde_json::json!({
                "choice": "yes", "probabilities": {"yes": 0.75, "no": 0.25}, "confidence": 0.5
            })
        ));
        assert!(!answer_matches(
            &question,
            &serde_json::json!({
                "choice": "yes", "probabilities": {"yes": 0.75, "no": 0.35}, "confidence": 0.5
            })
        ));
        assert!(!answer_matches(
            &question,
            &serde_json::json!({
                "choice": "unknown", "probabilities": {"yes": 0.75, "no": 0.25}, "confidence": 0.5
            })
        ));
    }

    #[test]
    fn persistent_decision_state_rejects_inline_media_fields() {
        let request = DecisionRequest {
            model: Some("decision".into()),
            state: serde_json::json!({"messages": [{"image_url": "data:image/png;base64,aGVsbG8="}]}),
            questions: BTreeMap::from([(
                "visible".into(),
                DecisionQuestion {
                    kind: DecisionQuestionKind::Noul,
                    instructions: None,
                    criteria: None,
                },
            )]),
        };
        assert_eq!(
            request.validate(),
            Err("decision media must use the ephemeral files field")
        );
    }
}
