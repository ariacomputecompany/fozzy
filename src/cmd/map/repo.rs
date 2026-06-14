use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::{FozzyError, FozzyResult};

use super::{
    HotspotSignals, MapHotspot, RepoFacts, ScanRecord, ServiceBoundary,
    recommended_suites_for_hotspot,
};

pub(crate) fn scan_repo(root: &Path) -> FozzyResult<RepoFacts> {
    if !root.exists() {
        return Err(FozzyError::InvalidArgument(format!(
            "map root does not exist: {}",
            root.display()
        )));
    }

    let mut records = Vec::<ScanRecord>::new();
    let mut scanned_files = 0usize;
    let mut skipped_source_files = Vec::new();
    let scan_roots = discover_scan_roots(root);
    if !scan_roots.iter().any(|scan_root| scan_root == root) {
        scan_root_level_files(
            root,
            &mut records,
            &mut scanned_files,
            &mut skipped_source_files,
        );
    }
    for scan_root in scan_roots {
        for entry in WalkDir::new(&scan_root)
            .into_iter()
            .filter_entry(|entry| should_descend(entry.path(), root))
            .flatten()
        {
            if !entry.file_type().is_file() {
                continue;
            }
            scan_candidate_file(
                root,
                entry.path(),
                &mut records,
                &mut scanned_files,
                &mut skipped_source_files,
            );
        }
    }

    let mut hotspots = records
        .iter()
        .map(|record| MapHotspot {
            id: format!("{}:{}", record.component, record.rel.display()),
            component: record.component.clone(),
            path: record.rel.display().to_string(),
            risk_score: record.risk_score,
            reasons: record.reasons.clone(),
            signals: record.signal.clone(),
            recommended_suites: recommended_suites_for_hotspot(&record.signal),
        })
        .collect::<Vec<_>>();
    hotspots.sort_by(|a, b| {
        b.risk_score
            .cmp(&a.risk_score)
            .then_with(|| a.path.cmp(&b.path))
    });

    let mut by_component = BTreeMap::<String, (usize, usize, usize, usize)>::new();
    for record in &records {
        let entry = by_component
            .entry(record.component.clone())
            .or_insert((0usize, 0usize, 0usize, 0usize));
        entry.0 += 1;
        entry.1 += record.signal.entrypoint_signals;
        entry.2 += record.signal.external_signals;
        entry.3 += record.signal.concurrency_signals;
    }

    let mut services = Vec::<ServiceBoundary>::new();
    for (name, (file_count, entrypoint, external, concurrency)) in by_component {
        if file_count < 2 {
            continue;
        }
        let kind = if entrypoint > 0 && external > 0 {
            "service"
        } else if concurrency > 0 {
            "worker"
        } else {
            "library"
        };
        services.push(ServiceBoundary {
            path: name.clone(),
            name,
            kind: kind.to_string(),
            file_count,
            entrypoint_signals: entrypoint,
            external_signals: external,
            concurrency_signals: concurrency,
        });
    }
    services.sort_by(|a, b| {
        b.file_count
            .cmp(&a.file_count)
            .then_with(|| a.path.cmp(&b.path))
    });

    Ok(RepoFacts {
        root: root.to_path_buf(),
        scanned_files,
        skipped_source_files,
        hotspots,
        services,
    })
}

pub(crate) fn hotspot_hints(hotspot: &MapHotspot) -> Vec<String> {
    let mut out = BTreeSet::<String>::new();
    out.insert(hotspot.component.to_ascii_lowercase());
    out.insert(hotspot.path.to_ascii_lowercase());
    if let Some(stem) = Path::new(&hotspot.path)
        .file_stem()
        .and_then(|stem| stem.to_str())
    {
        out.insert(stem.to_ascii_lowercase().replace('.', "-"));
        out.insert(stem.to_ascii_lowercase().replace('.', "_"));
    }
    out.into_iter().filter(|hint| hint.len() >= 3).collect()
}

pub(crate) fn discover_scan_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = WalkDir::new(root)
        .max_depth(3)
        .into_iter()
        .filter_entry(|entry| should_descend(entry.path(), root))
        .flatten()
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .filter(|path| is_likely_source_dir(path))
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();

    let mut minimal = Vec::<PathBuf>::new();
    for candidate in roots {
        if minimal.iter().any(|root| candidate.starts_with(root)) {
            continue;
        }
        minimal.push(candidate);
    }
    if minimal.is_empty() {
        vec![root.to_path_buf()]
    } else {
        minimal
    }
}

pub(crate) fn should_skip_path(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    parent.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(should_skip_dir_name)
    })
}

fn scan_root_level_files(
    root: &Path,
    records: &mut Vec<ScanRecord>,
    scanned_files: &mut usize,
    skipped_source_files: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        scan_candidate_file(
            root,
            &entry.path(),
            records,
            scanned_files,
            skipped_source_files,
        );
    }
}

fn scan_candidate_file(
    root: &Path,
    path: &Path,
    records: &mut Vec<ScanRecord>,
    scanned_files: &mut usize,
    skipped_source_files: &mut Vec<String>,
) {
    if should_skip_path(path) || !is_candidate_file(path) {
        return;
    }
    let Ok(file) = std::fs::File::open(path) else {
        skipped_source_files.push(format!("{}: failed to open", path.display()));
        return;
    };
    let mut signal = HotspotSignals::default();
    let mut line_count = 0usize;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        match line {
            Ok(line) => {
                line_count = line_count.saturating_add(1);
                accumulate_signals_line(&mut signal, &line);
            }
            Err(err) => {
                skipped_source_files
                    .push(format!("{}: failed to read line: {err}", path.display()));
                break;
            }
        }
    }
    signal.line_count = line_count;
    let rel = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    *scanned_files += 1;
    let (risk_score, reasons) = score_signals(&signal);
    if risk_score == 0 {
        return;
    }
    records.push(ScanRecord {
        component: component_for_path(&rel),
        rel,
        signal,
        risk_score,
        reasons,
    });
}

fn should_descend(path: &Path, root: &Path) -> bool {
    if path == root {
        return true;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| !should_skip_dir_name(name))
}

fn should_skip_dir_name(segment: &str) -> bool {
    [
        ".git",
        ".fozzy",
        ".tmp",
        ".cache",
        ".pnpm-store",
        ".yarn",
        ".venv",
        ".tox",
        ".turbo",
        "__pycache__",
        "build",
        "cache",
        "coverage",
        "dist",
        "node_modules",
        "out",
        "target",
        "temp",
        "tmp",
        "vendor",
        "venv",
    ]
    .iter()
    .any(|needle| segment.eq_ignore_ascii_case(needle))
}

fn is_likely_source_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            [
                "app", "apps", "bin", "cmd", "crate", "crates", "internal", "lib", "libs",
                "package", "packages", "pkg", "service", "services", "src",
            ]
            .iter()
            .any(|needle| name.eq_ignore_ascii_case(needle))
        })
}

pub(crate) fn is_candidate_file(path: &Path) -> bool {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "package-lock.json"
                    | "yarn.lock"
                    | "pnpm-lock.yaml"
                    | "npm-shrinkwrap.json"
                    | "bun.lockb"
                    | "bun.lock"
                    | "cargo.lock"
                    | "go.sum"
                    | "gemfile.lock"
                    | "pipfile.lock"
                    | "poetry.lock"
                    | "composer.lock"
            )
        })
    {
        return false;
    }
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let lower = name.to_ascii_lowercase();
            lower.ends_with(".lock")
                || lower.ends_with(".min.js")
                || lower.ends_with(".min.css")
                || lower.contains(".generated.")
                || lower.contains(".gen.")
        })
    {
        return false;
    }
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("dockerfile"))
    {
        return true;
    }
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "rs" | "go"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "py"
            | "java"
            | "kt"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "swift"
            | "rb"
            | "php"
            | "scala"
            | "sql"
            | "sh"
    )
}

pub(crate) fn accumulate_signals_line(signals: &mut HotspotSignals, line: &str) {
    let lower = line.to_ascii_lowercase();
    signals.branch_signals = signals.branch_signals.saturating_add(count_hits(
        &lower,
        &[" if ", " else ", " match ", " switch ", " case ", " catch "],
    ));
    signals.concurrency_signals = signals.concurrency_signals.saturating_add(count_hits(
        &lower,
        &[
            " async ",
            ".await",
            "thread",
            "mutex",
            "rwlock",
            "channel",
            "spawn",
            "tokio::",
            "select!",
            "goroutine",
            "go func",
        ],
    ));
    signals.external_signals = signals.external_signals.saturating_add(count_hits(
        &lower,
        &[
            "http://",
            "https://",
            "grpc",
            "sql",
            "redis",
            "kafka",
            "rabbit",
            "nats",
            "s3",
            "command::new",
            "std::fs",
            "subprocess",
            "socket",
            "database",
            "postgres",
            "mysql",
            "mongodb",
        ],
    ));
    signals.failure_signals = signals.failure_signals.saturating_add(count_hits(
        &lower,
        &[
            "timeout",
            "retry",
            "backoff",
            "circuit",
            "panic",
            "throw",
            "except",
            "rollback",
            "compensat",
            "fail",
            "error",
        ],
    ));
    signals.memory_signals = signals.memory_signals.saturating_add(count_hits(
        &lower,
        &["alloc", "free", "leak", "memory", "heap"],
    ));
    signals.entrypoint_signals = signals.entrypoint_signals.saturating_add(count_hits(
        &lower,
        &[
            "fn main",
            "main(",
            "applisten",
            "listen(",
            "router",
            "fastapi",
            "express(",
            "httpserver",
            "grpcserver",
            "deployment",
            "kind: service",
        ],
    ));
}

pub(crate) fn count_hits(haystack: &str, needles: &[&str]) -> usize {
    needles
        .iter()
        .map(|needle| haystack.matches(needle).count())
        .sum()
}

pub(crate) fn score_signals(signals: &HotspotSignals) -> (u8, Vec<String>) {
    let mut reasons = Vec::<String>::new();
    let mut score = 0usize;

    score += signals.branch_signals.min(30);
    if signals.branch_signals > 8 {
        reasons.push(format!("high branch density ({})", signals.branch_signals));
    }

    score += signals.concurrency_signals.saturating_mul(6).min(30);
    if signals.concurrency_signals > 0 {
        reasons.push(format!(
            "concurrency signals ({})",
            signals.concurrency_signals
        ));
    }

    score += signals.external_signals.saturating_mul(5).min(25);
    if signals.external_signals > 0 {
        reasons.push(format!(
            "external side-effect signals ({})",
            signals.external_signals
        ));
    }

    score += signals.failure_signals.saturating_mul(3).min(15);
    if signals.failure_signals > 3 {
        reasons.push(format!(
            "failure/timeout/retry signals ({})",
            signals.failure_signals
        ));
    }

    if signals.memory_signals > 2 {
        score += 8;
        reasons.push(format!(
            "memory management signals ({})",
            signals.memory_signals
        ));
    }

    if signals.entrypoint_signals > 0 {
        score += 5;
        reasons.push("service/entrypoint boundary indicators".to_string());
    }

    if signals.line_count > 500 {
        score += 7;
        reasons.push(format!("large file size ({} lines)", signals.line_count));
    } else if signals.line_count > 250 {
        score += 4;
    }

    (score.min(100) as u8, reasons)
}

pub(crate) fn component_for_path(rel: &Path) -> String {
    let parts: Vec<String> = rel
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(|segment| segment.to_ascii_lowercase())
        .collect();
    if parts.is_empty() {
        return "root".to_string();
    }
    for marker in ["services", "apps", "packages", "crates", "modules"] {
        if let Some(idx) = parts.iter().position(|part| part == marker)
            && let Some(next) = parts.get(idx + 1)
        {
            return format!("{marker}/{next}");
        }
    }
    parts.first().cloned().unwrap_or_else(|| "root".to_string())
}
