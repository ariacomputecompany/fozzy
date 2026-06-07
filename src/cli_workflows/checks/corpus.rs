use super::*;

pub(crate) fn corpus_add_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let Some(added) = value.get("added").and_then(|v| v.as_str()) else {
        return (
            FullStepStatus::Failed,
            "missing added path in corpus add response".to_string(),
        );
    };
    let added = PathBuf::from(added);
    file_artifact_status(&added)
}

pub(crate) fn listed_file_status(path: &Path) -> anyhow::Result<u64> {
    let metadata = std::fs::metadata(path)
        .map_err(|err| anyhow::anyhow!("{} missing: {err}", path.display()))?;
    anyhow::ensure!(metadata.is_file(), "{} is not a file", path.display());
    anyhow::ensure!(metadata.len() > 0, "{} is empty", path.display());
    Ok(metadata.len())
}

pub(crate) fn corpus_list_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let entries = value.as_array().cloned().unwrap_or_default();
    let count = entries.len();
    let mut invalid = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in &entries {
        let Some(path_str) = entry.as_str() else {
            invalid.push(format!("non-string entry: {}", entry));
            continue;
        };
        let trimmed = path_str.trim();
        if trimmed.is_empty() {
            invalid.push("blank entry path".to_string());
            continue;
        }
        if !seen.insert(trimmed.to_string()) {
            invalid.push(format!("duplicate entry path: {trimmed}"));
            continue;
        }
        if let Err(err) = listed_file_status(Path::new(trimmed)) {
            invalid.push(err.to_string());
        }
    }
    (
        if count > 0 && invalid.is_empty() {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        if invalid.is_empty() {
            format!("files={count} invalid=<none>")
        } else {
            format!("files={count} invalid={}", invalid.join("; "))
        },
    )
}

pub(crate) fn corpus_minimize_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let before = value
        .get("filesBefore")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let after = value
        .get("filesAfter")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let removed = value
        .get("duplicatesRemoved")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bytes_before = value
        .get("bytesBefore")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bytes_after = value
        .get("bytesAfter")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bytes_removed = value
        .get("bytesRemoved")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let file_math_ok =
        before > 0 && after > 0 && after <= before && removed == before.saturating_sub(after);
    let bytes_present = value.get("bytesBefore").is_some()
        || value.get("bytesAfter").is_some()
        || value.get("bytesRemoved").is_some();
    let byte_math_ok = !bytes_present
        || (bytes_before >= bytes_after
            && bytes_removed == bytes_before.saturating_sub(bytes_after));
    let ok = file_math_ok && byte_math_ok;
    (
        if ok {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "files_before={before} files_after={after} duplicates_removed={removed} bytes_before={bytes_before} bytes_after={bytes_after} bytes_removed={bytes_removed}"
        ),
    )
}

pub(crate) fn corpus_import_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let Some(import_dir) = value.get("dir").and_then(|v| v.as_str()) else {
        return (
            FullStepStatus::Failed,
            "missing dir path in corpus import response".to_string(),
        );
    };
    let path = Path::new(import_dir);
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            let mut entries = 0usize;
            let mut invalid = Vec::new();
            match std::fs::read_dir(path) {
                Ok(iter) => {
                    for entry in iter {
                        match entry {
                            Ok(entry) => {
                                entries += 1;
                                if let Err(err) = listed_file_status(&entry.path()) {
                                    invalid.push(err.to_string());
                                }
                            }
                            Err(err) => invalid
                                .push(format!("{} read_dir entry error: {err}", path.display())),
                        }
                    }
                }
                Err(err) => invalid.push(format!("{} read_dir error: {err}", path.display())),
            }
            (
                if entries > 0 && invalid.is_empty() {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                if invalid.is_empty() {
                    format!("path={} entries={} invalid=<none>", path.display(), entries)
                } else {
                    format!(
                        "path={} entries={} invalid={}",
                        path.display(),
                        entries,
                        invalid.join("; ")
                    )
                },
            )
        }
        Ok(_) => (
            FullStepStatus::Failed,
            format!("path={} is not a directory", path.display()),
        ),
        Err(err) => (
            FullStepStatus::Failed,
            format!("path={} missing: {err}", path.display()),
        ),
    }
}

pub(crate) fn artifacts_list_status(
    output: &fozzy::ArtifactOutput,
    fallback: &Path,
) -> (FullStepStatus, String) {
    match output {
        fozzy::ArtifactOutput::List { entries } => {
            let mut invalid = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for entry in entries {
                let trimmed = entry.path.trim();
                if trimmed.is_empty() {
                    invalid.push("blank artifact path".to_string());
                    continue;
                }
                if !seen.insert(trimmed.to_string()) {
                    invalid.push(format!("duplicate artifact path: {trimmed}"));
                    continue;
                }
                let path = Path::new(trimmed);
                match listed_file_status(path) {
                    Ok(size) => {
                        if let Some(reported) = entry.size_bytes
                            && reported != size
                        {
                            invalid.push(format!(
                                "{} size mismatch reported={} actual={}",
                                path.display(),
                                reported,
                                size
                            ));
                        }
                    }
                    Err(err) => invalid.push(err.to_string()),
                }
            }
            (
                if entries.is_empty() || !invalid.is_empty() {
                    FullStepStatus::Failed
                } else {
                    FullStepStatus::Passed
                },
                if invalid.is_empty() {
                    format!(
                        "entries={} run={} invalid=<none>",
                        entries.len(),
                        fallback.display()
                    )
                } else {
                    format!(
                        "entries={} run={} invalid={}",
                        entries.len(),
                        fallback.display(),
                        invalid.join("; ")
                    )
                },
            )
        }
        _ => (
            FullStepStatus::Failed,
            format!("unexpected artifacts ls payload for {}", fallback.display()),
        ),
    }
}

pub(crate) fn artifacts_diff_status(output: &fozzy::ArtifactOutput) -> (FullStepStatus, String) {
    match output {
        fozzy::ArtifactOutput::Diff { diff } => {
            let left_ok = !diff.left.trim().is_empty();
            let right_ok = !diff.right.trim().is_empty();
            let mut invalid = 0usize;
            let mut seen = std::collections::BTreeSet::new();
            for file in &diff.files {
                let trimmed = file.key.trim();
                let has_left = file.left_path.is_some();
                let has_right = file.right_path.is_some();
                let size_differs = file.left_size_bytes != file.right_size_bytes;
                let impossible_unchanged =
                    !file.changed && (size_differs || !has_left || !has_right);
                let duplicate = !trimmed.is_empty() && !seen.insert(trimmed.to_string());
                if trimmed.is_empty()
                    || (!has_left && !has_right)
                    || impossible_unchanged
                    || duplicate
                {
                    invalid += 1;
                }
            }
            let evidence_count = diff.files.len()
                + usize::from(diff.report.is_some())
                + usize::from(diff.trace.is_some());
            (
                if evidence_count > 0 && left_ok && right_ok && invalid == 0 {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                format!(
                    "left={} left_ok={} right={} right_ok={} file_deltas={} report={} trace={} invalid={}",
                    diff.left,
                    left_ok,
                    diff.right,
                    right_ok,
                    diff.files.len(),
                    diff.report.is_some(),
                    diff.trace.is_some(),
                    invalid
                ),
            )
        }
        _ => (
            FullStepStatus::Failed,
            "unexpected artifacts diff payload".to_string(),
        ),
    }
}
