#[test]
fn crate_exposes_a_phase_one_contract_marker() {
    assert_eq!(greatgramma_core::PHASE, "phase-1-bootstrap");
}
