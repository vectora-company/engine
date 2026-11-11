use vectora_engine::{
    harness::{
        EvidenceDispositionPolicy, RunIdentity, RunLifecycle, RunStatus, VerificationReport,
        evaluate_verification,
    },
    storage::StorageSelection,
};

const FIXTURE: &str = include_str!("../../../tests/fixtures/conformance-v1.json");

#[test]
fn shared_json_fixture_conforms_to_run_verification_and_storage_contracts() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).expect("valid fixture JSON");

    let identity: RunIdentity =
        serde_json::from_value(fixture["run_lifecycle"]["identity"].clone())
            .expect("run identity fixture");
    let mut run = RunLifecycle::new(identity).expect("planned run");
    assert_eq!(
        serde_json::to_value(&run).unwrap(),
        fixture["run_lifecycle"]["planned"]
    );
    run.transition(RunStatus::Running)
        .expect("valid transition");
    assert_eq!(
        serde_json::to_value(&run).unwrap(),
        fixture["run_lifecycle"]["running"]
    );

    let report: VerificationReport =
        serde_json::from_value(fixture["verification"]["report"].clone())
            .expect("verification report fixture");
    let policy: EvidenceDispositionPolicy =
        serde_json::from_value(fixture["verification"]["policy"].clone())
            .expect("verification policy fixture");
    let evaluation = evaluate_verification(&report, &policy).expect("verification evaluation");
    assert_eq!(
        serde_json::to_value(evaluation).unwrap(),
        fixture["verification"]["evaluation"]
    );

    let selection: StorageSelection = serde_json::from_value(fixture["storage_selection"].clone())
        .expect("storage selection fixture");
    selection.validate().expect("supported storage selection");
    assert_eq!(
        serde_json::to_value(selection).unwrap(),
        fixture["storage_selection"]
    );
}
