//! Verification evidence and gate decisions, kept distinct from run status.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Version of verification result and quality gate contracts.
pub const EVIDENCE_GATE_CONTRACT_VERSION: u16 = 1;

/// Result reported by one consumer-provided verification check.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationOutcome {
    Passed,
    Failed,
    InfraError,
    NotRun,
}

/// Aggregate result for the required checks in a verification report.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Passed,
    Failed,
    InfraError,
    NotRun,
}

/// Final decision made by a gate policy from verification results.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    Passed,
    Degraded,
    Blocked,
    Unverified,
}

/// Gate disposition for a failed required check.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureDisposition {
    Degraded,
    Blocked,
}

/// Gate disposition when a required result or evidence is inconclusive.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDisposition {
    Degraded,
    Blocked,
    Unverified,
}

/// Pointer to a redacted artifact containing concrete verification evidence.
/// The artifact body is owned and redacted by the consumer, never by the model.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationEvidence {
    pub evidence_id: String,
    pub origin: String,
    pub artifact_ref: String,
    #[serde(default)]
    pub sha256: Option<String>,
    pub redacted: bool,
    #[serde(default)]
    pub expires_at_unix_ms: Option<u64>,
}

/// One product-configured verification check and its evidence references.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCheck {
    pub check_id: String,
    pub outcome: VerificationOutcome,
    #[serde(default)]
    pub evidence: Vec<VerificationEvidence>,
}

/// Consumer-supplied results for one run. It contains references, not artifact
/// contents, logs, model reasoning, or secrets.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationReport {
    pub contract_version: u16,
    pub run_id: String,
    pub evaluated_at_unix_ms: u64,
    pub checks: Vec<VerificationCheck>,
}

/// Explicit, deterministic policy for required checks and missing evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceDispositionPolicy {
    pub contract_version: u16,
    pub required_checks: Vec<String>,
    pub require_evidence: bool,
    pub failed_check: FailureDisposition,
    pub inconclusive: EvidenceDisposition,
}

/// Verification status, gate verdict and stable reason code.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GateEvaluation {
    pub contract_version: u16,
    pub run_id: String,
    pub verification_status: VerificationStatus,
    pub verdict: GateVerdict,
    pub reason_code: String,
}

/// Invalid or inconsistent verification input.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum VerificationError {
    #[error("unsupported evidence gate contract version {received}")]
    UnsupportedContractVersion { received: u16 },
    #[error("verification run ID must not be empty")]
    EmptyRunId,
    #[error("verification timestamp must be positive")]
    InvalidTimestamp,
    #[error("verification report must contain at least one check")]
    EmptyReport,
    #[error("verification check IDs must be non-empty and unique")]
    InvalidCheckId,
    #[error("gate policy must contain unique non-empty required check IDs")]
    InvalidRequiredChecks,
    #[error("required verification check is missing: {0}")]
    MissingRequiredCheck(String),
    #[error("evidence IDs must be non-empty and unique")]
    InvalidEvidenceId,
    #[error("evidence origin and artifact reference must not be empty")]
    InvalidEvidenceReference,
    #[error("evidence digest must be a 64 character hexadecimal SHA-256")]
    InvalidEvidenceDigest,
    #[error("evidence must be redacted before it is attached to a report")]
    UnredactedEvidence,
}

/// Evaluate a report using a supplied clock value embedded in that report.
/// Required failed checks take precedence over infrastructure errors, which
/// take precedence over missing/not-run checks and evidence.
pub fn evaluate_verification(
    report: &VerificationReport,
    policy: &EvidenceDispositionPolicy,
) -> Result<GateEvaluation, VerificationError> {
    validate_report(report, policy)?;

    let required: HashSet<&str> = policy.required_checks.iter().map(String::as_str).collect();
    let checks: Vec<&VerificationCheck> = report
        .checks
        .iter()
        .filter(|check| required.contains(check.check_id.as_str()))
        .collect();

    let (verification_status, verdict, reason_code) = if checks
        .iter()
        .any(|check| check.outcome == VerificationOutcome::Failed)
    {
        (
            VerificationStatus::Failed,
            match policy.failed_check {
                FailureDisposition::Blocked => GateVerdict::Blocked,
                FailureDisposition::Degraded => GateVerdict::Degraded,
            },
            "required_check_failed",
        )
    } else if checks
        .iter()
        .any(|check| check.outcome == VerificationOutcome::InfraError)
    {
        (
            VerificationStatus::InfraError,
            inconclusive_verdict(policy.inconclusive),
            "verification_infrastructure_error",
        )
    } else if checks
        .iter()
        .any(|check| check.outcome == VerificationOutcome::NotRun)
    {
        (
            VerificationStatus::NotRun,
            inconclusive_verdict(policy.inconclusive),
            "required_check_not_run",
        )
    } else if policy.require_evidence
        && checks.iter().any(|check| {
            check.outcome == VerificationOutcome::Passed
                && !check.evidence.iter().any(|evidence| {
                    evidence
                        .expires_at_unix_ms
                        .is_none_or(|expiry| expiry > report.evaluated_at_unix_ms)
                })
        })
    {
        (
            VerificationStatus::NotRun,
            inconclusive_verdict(policy.inconclusive),
            "required_evidence_missing_or_expired",
        )
    } else {
        (
            VerificationStatus::Passed,
            GateVerdict::Passed,
            "all_required_checks_passed",
        )
    };

    Ok(GateEvaluation {
        contract_version: EVIDENCE_GATE_CONTRACT_VERSION,
        run_id: report.run_id.clone(),
        verification_status,
        verdict,
        reason_code: reason_code.into(),
    })
}

fn inconclusive_verdict(disposition: EvidenceDisposition) -> GateVerdict {
    match disposition {
        EvidenceDisposition::Blocked => GateVerdict::Blocked,
        EvidenceDisposition::Degraded => GateVerdict::Degraded,
        EvidenceDisposition::Unverified => GateVerdict::Unverified,
    }
}

fn validate_report(
    report: &VerificationReport,
    policy: &EvidenceDispositionPolicy,
) -> Result<(), VerificationError> {
    if report.contract_version != EVIDENCE_GATE_CONTRACT_VERSION {
        return Err(VerificationError::UnsupportedContractVersion {
            received: report.contract_version,
        });
    }
    if report.run_id.trim().is_empty() {
        return Err(VerificationError::EmptyRunId);
    }
    if report.evaluated_at_unix_ms == 0 {
        return Err(VerificationError::InvalidTimestamp);
    }
    if report.checks.is_empty() {
        return Err(VerificationError::EmptyReport);
    }
    if policy.contract_version != EVIDENCE_GATE_CONTRACT_VERSION {
        return Err(VerificationError::UnsupportedContractVersion {
            received: policy.contract_version,
        });
    }
    if policy.required_checks.is_empty() {
        return Err(VerificationError::InvalidRequiredChecks);
    }
    let mut required = HashSet::new();
    if policy
        .required_checks
        .iter()
        .any(|id| id.trim().is_empty() || !required.insert(id.as_str()))
    {
        return Err(VerificationError::InvalidRequiredChecks);
    }
    let mut check_ids = HashSet::new();
    let mut evidence_ids = HashSet::new();
    for check in &report.checks {
        if check.check_id.trim().is_empty() || !check_ids.insert(check.check_id.as_str()) {
            return Err(VerificationError::InvalidCheckId);
        }
        for evidence in &check.evidence {
            if evidence.evidence_id.trim().is_empty()
                || !evidence_ids.insert(evidence.evidence_id.as_str())
            {
                return Err(VerificationError::InvalidEvidenceId);
            }
            if evidence.origin.trim().is_empty() || evidence.artifact_ref.trim().is_empty() {
                return Err(VerificationError::InvalidEvidenceReference);
            }
            if let Some(digest) = evidence.sha256.as_deref()
                && (digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
            {
                return Err(VerificationError::InvalidEvidenceDigest);
            }
            if !evidence.redacted {
                return Err(VerificationError::UnredactedEvidence);
            }
        }
    }
    for required_id in &policy.required_checks {
        if !check_ids.contains(required_id.as_str()) {
            return Err(VerificationError::MissingRequiredCheck(required_id.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(id: &str, expires_at_unix_ms: Option<u64>) -> VerificationEvidence {
        VerificationEvidence {
            evidence_id: id.into(),
            origin: "local_test_runner".into(),
            artifact_ref: format!("artifact:{id}"),
            sha256: Some("a".repeat(64)),
            redacted: true,
            expires_at_unix_ms,
        }
    }

    fn check(id: &str, outcome: VerificationOutcome) -> VerificationCheck {
        VerificationCheck {
            check_id: id.into(),
            outcome,
            evidence: vec![evidence(&format!("evidence-{id}"), None)],
        }
    }

    fn policy() -> EvidenceDispositionPolicy {
        EvidenceDispositionPolicy {
            contract_version: EVIDENCE_GATE_CONTRACT_VERSION,
            required_checks: vec!["tests".into(), "security".into()],
            require_evidence: true,
            failed_check: FailureDisposition::Blocked,
            inconclusive: EvidenceDisposition::Unverified,
        }
    }

    fn report(checks: Vec<VerificationCheck>) -> VerificationReport {
        VerificationReport {
            contract_version: EVIDENCE_GATE_CONTRACT_VERSION,
            run_id: "run-1".into(),
            evaluated_at_unix_ms: 100,
            checks,
        }
    }

    #[test]
    fn separates_check_status_from_gate_verdict_and_ignores_optional_checks() {
        let result = evaluate_verification(
            &report(vec![
                check("tests", VerificationOutcome::Passed),
                check("security", VerificationOutcome::Passed),
                check("optional", VerificationOutcome::Failed),
            ]),
            &policy(),
        )
        .unwrap();
        assert_eq!(result.verification_status, VerificationStatus::Passed);
        assert_eq!(result.verdict, GateVerdict::Passed);
        assert_eq!(result.reason_code, "all_required_checks_passed");
    }

    #[test]
    fn required_failures_block_before_infrastructure_or_not_run_results() {
        let result = evaluate_verification(
            &report(vec![
                check("tests", VerificationOutcome::InfraError),
                check("security", VerificationOutcome::Failed),
            ]),
            &policy(),
        )
        .unwrap();
        assert_eq!(result.verification_status, VerificationStatus::Failed);
        assert_eq!(result.verdict, GateVerdict::Blocked);
    }

    #[test]
    fn infrastructure_and_not_run_never_count_as_passed() {
        for (outcome, expected, reason) in [
            (
                VerificationOutcome::InfraError,
                VerificationStatus::InfraError,
                "verification_infrastructure_error",
            ),
            (
                VerificationOutcome::NotRun,
                VerificationStatus::NotRun,
                "required_check_not_run",
            ),
        ] {
            let result = evaluate_verification(
                &report(vec![
                    check("tests", outcome),
                    check("security", VerificationOutcome::Passed),
                ]),
                &policy(),
            )
            .unwrap();
            assert_eq!(result.verification_status, expected);
            assert_eq!(result.verdict, GateVerdict::Unverified);
            assert_eq!(result.reason_code, reason);
        }
    }

    #[test]
    fn missing_or_expired_evidence_is_unverified() {
        let mut tests = check("tests", VerificationOutcome::Passed);
        tests.evidence.clear();
        let result = evaluate_verification(
            &report(vec![tests, check("security", VerificationOutcome::Passed)]),
            &policy(),
        )
        .unwrap();
        assert_eq!(result.verification_status, VerificationStatus::NotRun);
        assert_eq!(result.verdict, GateVerdict::Unverified);

        let mut expired = check("tests", VerificationOutcome::Passed);
        expired.evidence[0].expires_at_unix_ms = Some(100);
        let result = evaluate_verification(
            &report(vec![
                expired,
                check("security", VerificationOutcome::Passed),
            ]),
            &policy(),
        )
        .unwrap();
        assert_eq!(result.reason_code, "required_evidence_missing_or_expired");
    }

    #[test]
    fn rejects_missing_required_checks_and_unredacted_evidence() {
        assert!(matches!(
            evaluate_verification(&report(vec![check("tests", VerificationOutcome::Passed)]), &policy()),
            Err(VerificationError::MissingRequiredCheck(id)) if id == "security"
        ));
        let mut unredacted = check("tests", VerificationOutcome::Passed);
        unredacted.evidence[0].redacted = false;
        assert_eq!(
            evaluate_verification(
                &report(vec![
                    unredacted,
                    check("security", VerificationOutcome::Passed)
                ]),
                &policy()
            ),
            Err(VerificationError::UnredactedEvidence)
        );
    }

    #[test]
    fn rejects_empty_report() {
        assert_eq!(
            evaluate_verification(&report(Vec::new()), &policy()),
            Err(VerificationError::EmptyReport)
        );
    }

    #[test]
    fn validates_digest_and_policy_dispositions() {
        let mut invalid = check("tests", VerificationOutcome::Passed);
        invalid.evidence[0].sha256 = Some("bad".into());
        assert_eq!(
            evaluate_verification(
                &report(vec![
                    invalid,
                    check("security", VerificationOutcome::Passed)
                ]),
                &policy()
            ),
            Err(VerificationError::InvalidEvidenceDigest)
        );

        let mut degraded = policy();
        degraded.failed_check = FailureDisposition::Degraded;
        let result = evaluate_verification(
            &report(vec![
                check("tests", VerificationOutcome::Failed),
                check("security", VerificationOutcome::Passed),
            ]),
            &degraded,
        )
        .unwrap();
        assert_eq!(result.verdict, GateVerdict::Degraded);
    }
}
