use crate::FullStepStatus;
use crate::cli_workflows::*;

#[test]
fn ci_report_status_surfaces_failing_check_detail() {
    let report = fozzy::CiReport {
        schema_version: "fozzy.ci_report.v1".to_string(),
        ok: false,
        checks: vec![
            fozzy::CiCheck {
                name: "trace_verify".to_string(),
                ok: true,
                detail: Some(
                    "checksum_present=true checksum_valid=true warnings=<none>".to_string(),
                ),
            },
            fozzy::CiCheck {
                name: "strict_warning_policy".to_string(),
                ok: false,
                detail: Some(
                    "strict=true warnings=[\"detected 1 leaked allocation(s)\"]".to_string(),
                ),
            },
        ],
    };
    let (status, detail) = ci_report_status(&report);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("checks=2"));
    assert!(detail.contains("strict_warning_policy: strict=true warnings="));
}

#[test]
fn ci_report_status_rejects_inconsistent_ok_summary() {
    let report = fozzy::CiReport {
        schema_version: "fozzy.ci_report.v1".to_string(),
        ok: true,
        checks: vec![fozzy::CiCheck {
            name: "trace_verify".to_string(),
            ok: false,
            detail: Some("checksum_valid=false".to_string()),
        }],
    };
    let (status, detail) = ci_report_status(&report);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("reported_ok=true"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn ci_report_status_rejects_invalid_check_names() {
    let report = fozzy::CiReport {
        schema_version: "fozzy.ci_report.v1".to_string(),
        ok: true,
        checks: vec![fozzy::CiCheck {
            name: "   ".to_string(),
            ok: true,
            detail: Some("ok".to_string()),
        }],
    };
    let (status, detail) = ci_report_status(&report);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid=1"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn ci_report_status_rejects_unknown_check_names() {
    let report = fozzy::CiReport {
        schema_version: "fozzy.ci_report.v1".to_string(),
        ok: true,
        checks: vec![fozzy::CiCheck {
            name: "mystery_check".to_string(),
            ok: true,
            detail: Some("ok".to_string()),
        }],
    };
    let (status, detail) = ci_report_status(&report);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid=1"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn ci_report_status_rejects_duplicate_check_names() {
    let report = fozzy::CiReport {
        schema_version: "fozzy.ci_report.v1".to_string(),
        ok: true,
        checks: vec![
            fozzy::CiCheck {
                name: "trace_verify".to_string(),
                ok: true,
                detail: Some("ok".to_string()),
            },
            fozzy::CiCheck {
                name: "trace_verify".to_string(),
                ok: true,
                detail: Some("ok again".to_string()),
            },
        ],
    };
    let (status, detail) = ci_report_status(&report);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate=1"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn ci_report_status_rejects_empty_check_set() {
    let report = fozzy::CiReport {
        schema_version: "fozzy.ci_report.v1".to_string(),
        ok: true,
        checks: vec![],
    };
    let (status, detail) = ci_report_status(&report);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("checks=0"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_surfaces_issue_and_hint() {
    let report = fozzy::DoctorReport {
        ok: false,
        issues: vec![fozzy::DoctorIssue {
            code: "proc_unmatched_preflight".to_string(),
            message: "strict proc backend preflight found an undeclared subprocess".to_string(),
            hint: Some("Add a `proc_when` step".to_string()),
            details: None,
        }],
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: true,
            signatures: vec!["abc".to_string(), "abc".to_string()],
            first_mismatch_run: None,
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("issues=1"));
    assert!(detail.contains(
        "proc_unmatched_preflight: strict proc backend preflight found an undeclared subprocess"
    ));
    assert!(detail.contains("Add a `proc_when` step"));
}

#[test]
fn doctor_report_status_rejects_inconsistent_ok_summary() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: vec![fozzy::DoctorIssue {
            code: "determinism_audit_mismatch".to_string(),
            message: "mismatch".to_string(),
            hint: None,
            details: None,
        }],
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: false,
            signatures: vec!["abc".to_string(), "def".to_string()],
            first_mismatch_run: Some(2),
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("reported_ok=true"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_zero_run_count() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: Vec::new(),
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 0,
            seed: 7,
            consistent: true,
            signatures: Vec::new(),
            first_mismatch_run: None,
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 0, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("runs=0"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_invalid_issue_rows() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: vec![fozzy::DoctorIssue {
            code: "".to_string(),
            message: " ".to_string(),
            hint: None,
            details: None,
        }],
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: true,
            signatures: vec!["abc".to_string(), "abc".to_string()],
            first_mismatch_run: None,
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_issues=1"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_unknown_issue_codes() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: vec![fozzy::DoctorIssue {
            code: "mystery_issue".to_string(),
            message: "unexpected".to_string(),
            hint: None,
            details: None,
        }],
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: true,
            signatures: vec!["abc".to_string(), "abc".to_string()],
            first_mismatch_run: None,
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_issues=1"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_duplicate_issue_rows() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: vec![
            fozzy::DoctorIssue {
                code: "determinism_audit_mismatch".to_string(),
                message: "mismatch".to_string(),
                hint: None,
                details: None,
            },
            fozzy::DoctorIssue {
                code: "determinism_audit_mismatch".to_string(),
                message: "mismatch".to_string(),
                hint: Some("same issue repeated".to_string()),
                details: None,
            },
        ],
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: false,
            signatures: vec!["abc".to_string(), "def".to_string()],
            first_mismatch_run: Some(2),
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_issues=1"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_invalid_signal_rows() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: Vec::new(),
        nondeterminism_signals: Some(vec![fozzy::NondeterminismSignal {
            source: "".to_string(),
            detail: "".to_string(),
        }]),
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: true,
            signatures: vec!["abc".to_string(), "abc".to_string()],
            first_mismatch_run: None,
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_signals=1"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_unknown_signal_sources() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: Vec::new(),
        nondeterminism_signals: Some(vec![fozzy::NondeterminismSignal {
            source: "stdout".to_string(),
            detail: "line ordering drift".to_string(),
        }]),
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: true,
            signatures: vec!["abc".to_string(), "abc".to_string()],
            first_mismatch_run: None,
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_signals=1"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_duplicate_signal_rows() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: Vec::new(),
        nondeterminism_signals: Some(vec![
            fozzy::NondeterminismSignal {
                source: "env".to_string(),
                detail: "line ordering drift".to_string(),
            },
            fozzy::NondeterminismSignal {
                source: "env".to_string(),
                detail: "line ordering drift".to_string(),
            },
        ]),
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: true,
            signatures: vec!["abc".to_string(), "abc".to_string()],
            first_mismatch_run: None,
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_signals=1"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_missing_determinism_audit() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: Vec::new(),
        nondeterminism_signals: None,
        determinism_audit: None,
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("audit_present=false"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_incoherent_determinism_audit() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: Vec::new(),
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/other.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: true,
            signatures: vec!["abc".to_string()],
            first_mismatch_run: Some(2),
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("audit_present=true"));
    assert!(detail.contains("audit_valid=false"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_mismatched_determinism_audit_seed() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: Vec::new(),
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 99,
            consistent: true,
            signatures: vec!["abc".to_string(), "abc".to_string()],
            first_mismatch_run: None,
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("audit_valid=false"));
    assert!(detail.contains("seed=7"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_missing_mismatch_issue_for_inconsistent_audit() {
    let report = fozzy::DoctorReport {
        ok: true,
        issues: Vec::new(),
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: false,
            signatures: vec!["abc".to_string(), "def".to_string()],
            first_mismatch_run: Some(2),
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("audit_issue_consistent=false"));
    assert!(detail.contains("derived_ok=false"));
}

#[test]
fn doctor_report_status_rejects_spurious_mismatch_issue_for_consistent_audit() {
    let report = fozzy::DoctorReport {
        ok: false,
        issues: vec![fozzy::DoctorIssue {
            code: "determinism_audit_mismatch".to_string(),
            message: "mismatch".to_string(),
            hint: None,
            details: None,
        }],
        nondeterminism_signals: None,
        determinism_audit: Some(fozzy::DeterminismAudit {
            scenario: "tests/repro.fozzy.json".to_string(),
            runs: 2,
            seed: 7,
            consistent: true,
            signatures: vec!["abc".to_string(), "abc".to_string()],
            first_mismatch_run: None,
        }),
    };
    let scenario = Path::new("tests/repro.fozzy.json");
    let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("audit_issue_consistent=false"));
    assert!(detail.contains("derived_ok=false"));
}
