use std::path::{Path, PathBuf};

use crate::{Config, FozzyResult};
use crate::{validate_direct_trace_bundle, validate_run_bundle_integrity};

use super::list::{artifacts_list, is_direct_trace_input, resolve_trace_path};
use super::output::{export_artifacts_dir_exact, export_artifacts_zip};

pub(super) fn export_reproducer_pack(config: &Config, run: &str, out: &Path) -> FozzyResult<()> {
    let strict_bundle = !is_direct_trace_input(run);
    let entries = artifacts_list(config, run)?;
    let mut files: Vec<PathBuf> = entries
        .into_iter()
        .map(|e| PathBuf::from(e.path))
        .filter(|p| p.exists() && p.is_file())
        .collect();
    files.sort();
    files.dedup();

    if files.is_empty() {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "no artifacts found for {run:?}"
        )));
    }
    if strict_bundle {
        validate_run_bundle_integrity(&files, run)?;
    } else {
        let trace_path = crate::normalize_trace_path(&PathBuf::from(run));
        let trusted_artifacts_dir = crate::trusted_explicit_trace_artifacts_dir(&trace_path)?;
        validate_direct_trace_bundle(&files, run, trusted_artifacts_dir.is_some())?;
    }

    let meta_dir = std::env::temp_dir().join(format!("fozzy-pack-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&meta_dir)?;
    let meta_files = vec![
        (
            "env.json",
            serde_json::to_vec_pretty(&crate::env_info(config))?,
        ),
        (
            "version.json",
            serde_json::to_vec_pretty(&crate::version_info())?,
        ),
        (
            "commandline.json",
            serde_json::to_vec_pretty(&serde_json::json!({
                "command": "fozzy artifacts pack",
                "target": run,
            }))?,
        ),
    ];
    for (name, bytes) in meta_files {
        std::fs::write(meta_dir.join(name), bytes)?;
    }
    files.push(meta_dir.join("env.json"));
    files.push(meta_dir.join("version.json"));
    files.push(meta_dir.join("commandline.json"));

    let res = if out
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("zip"))
    {
        export_artifacts_zip(&files, out)
    } else {
        export_artifacts_dir_exact(&files, out)
    };
    let _ = std::fs::remove_dir_all(meta_dir);
    res
}

pub(super) fn export_gate_bundle(config: &Config, run: &str, out: &Path) -> FozzyResult<()> {
    let direct_trace_input = is_direct_trace_input(run);
    let trace_path = resolve_trace_path(config, run)?;
    let trace_input = trace_path.to_string_lossy().to_string();
    let mut source_files: Vec<PathBuf> = vec![trace_path.clone()];
    if direct_trace_input {
        let trusted_artifacts_dir = crate::trusted_explicit_trace_artifacts_dir(&trace_path)?;
        if let Some(artifacts_dir) = trusted_artifacts_dir.clone() {
            for name in ["report.json", "manifest.json"] {
                let path = artifacts_dir.join(name);
                if path.exists() && path.is_file() {
                    source_files.push(path);
                }
            }
        }
        validate_direct_trace_bundle(&source_files, run, trusted_artifacts_dir.is_some())?;
    } else {
        let artifacts_dir = crate::resolve_artifacts_dir(config, run)?;
        for name in ["report.json", "manifest.json"] {
            let path = artifacts_dir.join(name);
            if path.exists() && path.is_file() {
                source_files.push(path);
            }
        }
        source_files.sort();
        source_files.dedup();
        validate_run_bundle_integrity(&source_files, run)?;
    }
    source_files.sort();
    source_files.dedup();

    let replay = crate::replay_trace(
        config,
        crate::TracePath::new(trace_path.clone()),
        &crate::ReplayOptions {
            step: false,
            until: None,
            dump_events: false,
            profile_capture: crate::ProfileCaptureLevel::Baseline,
            reporter: crate::Reporter::Json,
        },
    )?;
    let ci = crate::ci_evaluate(
        config,
        &crate::CiOptions {
            trace: trace_path.clone(),
            flake_runs: Vec::new(),
            flake_budget_pct: None,
            perf_baseline: None,
            max_p99_delta_pct: None,
            strict: true,
        },
    )?;
    let verify = crate::verify_trace_file(&trace_path)?;

    let mut files = source_files;

    let meta_dir = std::env::temp_dir().join(format!("fozzy-bundle-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&meta_dir)?;
    let meta_files = vec![
        (
            "env.json",
            serde_json::to_vec_pretty(&crate::env_info(config))?,
        ),
        (
            "version.json",
            serde_json::to_vec_pretty(&crate::version_info())?,
        ),
        (
            "trace_verify.report.json",
            serde_json::to_vec_pretty(&verify)?,
        ),
        (
            "replay.report.json",
            serde_json::to_vec_pretty(&replay.summary)?,
        ),
        ("ci.report.json", serde_json::to_vec_pretty(&ci)?),
        (
            "bundle.json",
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "fozzy.bundle_report.v1",
                "source": run,
                "trace": trace_input,
                "ciOk": ci.ok
            }))?,
        ),
    ];
    for (name, bytes) in meta_files {
        std::fs::write(meta_dir.join(name), bytes)?;
    }
    for name in [
        "env.json",
        "version.json",
        "trace_verify.report.json",
        "replay.report.json",
        "ci.report.json",
        "bundle.json",
    ] {
        files.push(meta_dir.join(name));
    }
    files.sort();
    files.dedup();
    let res = if out
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("zip"))
    {
        export_artifacts_zip(&files, out)
    } else {
        export_artifacts_dir_exact(&files, out)
    };
    let _ = std::fs::remove_dir_all(meta_dir);
    res
}

pub(super) fn export_artifacts(config: &Config, run: &str, out: &Path) -> FozzyResult<()> {
    let strict_bundle = !is_direct_trace_input(run);
    let entries = artifacts_list(config, run)?;
    let mut files: Vec<PathBuf> = entries
        .into_iter()
        .map(|e| PathBuf::from(e.path))
        .filter(|p| p.exists() && p.is_file())
        .collect();
    files.sort();
    files.dedup();

    if files.is_empty() {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "no artifacts found for {run:?}"
        )));
    }
    if strict_bundle {
        validate_run_bundle_integrity(&files, run)?;
    } else {
        let trace_path = crate::normalize_trace_path(&PathBuf::from(run));
        let trusted_artifacts_dir = crate::trusted_explicit_trace_artifacts_dir(&trace_path)?;
        validate_direct_trace_bundle(&files, run, trusted_artifacts_dir.is_some())?;
    }

    if out
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("zip"))
    {
        export_artifacts_zip(&files, out)?;
        return Ok(());
    }

    export_artifacts_dir_exact(&files, out)
}
