//! Process-level contract for the pre-product status executable.

use std::process::Command;

#[test]
fn t4_e1_status_executable_reports_the_contract_baseline() {
    let output = Command::new(env!("CARGO_BIN_EXE_study-tts-cli"))
        .output()
        .expect("the status executable should start");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("status output should be UTF-8"),
        "study-tts E1-S1: tested contract baseline available; \
         product CLI commands begin at E1-S5\n"
    );
    assert!(output.stderr.is_empty());
}
