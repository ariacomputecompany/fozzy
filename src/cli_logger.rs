use anyhow::Result;
use fozzy::{RunSummary, UsageDoc};
use serde::Serialize;
use serde_json::Value;

pub struct CliLogger {
    json: bool,
    no_color: bool,
}

impl CliLogger {
    pub fn new(json: bool, no_color: bool) -> Self {
        Self { json, no_color }
    }

    pub fn print_serialized<T: Serialize>(&self, value: &T) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string(value)?);
            return Ok(());
        }

        let rendered = render_value(&serde_json::to_value(value)?, 0);
        println!("{rendered}");
        Ok(())
    }

    pub fn print_run_summary(&self, summary: &RunSummary) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string(summary)?);
            return Ok(());
        }

        let status_style = match summary.status {
            fozzy::ExitStatus::Pass => self.style("PASS", "32;1"),
            fozzy::ExitStatus::Fail => self.style("FAIL", "31;1"),
            fozzy::ExitStatus::Timeout => self.style("TIMEOUT", "33;1"),
            fozzy::ExitStatus::Crash => self.style("CRASH", "31;1"),
            fozzy::ExitStatus::Error => self.style("ERROR", "31;1"),
        };

        let mut out = String::new();
        out.push_str(&format!(
            "{} {} {}\n",
            self.style("fozzy", "36;1"),
            self.style(&format!("{:?}", summary.mode).to_lowercase(), "37;1"),
            status_style
        ));
        out.push_str(&format!(
            "{} {}\n",
            self.style("run", "90"),
            summary.identity.run_id
        ));
        out.push_str(&format!(
            "{} {}\n",
            self.style("seed", "90"),
            summary.identity.seed
        ));
        out.push_str(&format!(
            "{} {}ms\n",
            self.style("duration", "90"),
            summary.duration_ms
        ));

        if let Some(tests) = &summary.tests {
            out.push_str(&format!(
                "{} pass={} fail={} skip={}\n",
                self.style("tests", "90"),
                tests.passed,
                tests.failed,
                tests.skipped
            ));
        }

        if let Some(mem) = &summary.memory {
            out.push_str(&format!(
                "{} allocs={} frees={} failed={} in_use={} peak={} leaked={} leaked_allocs={}\n",
                self.style("memory", "90"),
                mem.alloc_count,
                mem.free_count,
                mem.failed_alloc_count,
                mem.in_use_bytes,
                mem.peak_bytes,
                mem.leaked_bytes,
                mem.leaked_allocs
            ));
        }

        if let Some(path) = &summary.identity.trace_path {
            out.push_str(&format!("{} {}\n", self.style("trace", "90"), path));
        }
        if let Some(path) = &summary.identity.report_path {
            out.push_str(&format!("{} {}\n", self.style("report", "90"), path));
        }
        if let Some(path) = &summary.identity.artifacts_dir {
            out.push_str(&format!("{} {}\n", self.style("artifacts", "90"), path));
        }

        if !summary.findings.is_empty() {
            out.push_str(&format!("{}\n", self.style("findings", "33;1")));
            for finding in &summary.findings {
                out.push_str(&format!(
                    "  - [{}] {}: {}\n",
                    format!("{:?}", finding.kind).to_lowercase(),
                    finding.title,
                    finding.message
                ));
            }
        }

        println!("{}", out.trim_end());
        Ok(())
    }

    pub fn print_usage(&self, doc: &UsageDoc) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string(doc)?);
            return Ok(());
        }

        let mut out = String::new();
        out.push_str(&format!("{}\n", self.style(&doc.title, "36;1")));
        out.push('\n');
        for item in &doc.items {
            out.push_str(&format!("{}\n", self.style(&item.command, "37;1")));
            out.push_str(&format!("  {} {}\n", self.style("when", "90"), item.when));
            out.push_str(&format!("  {} {}\n", self.style("how", "90"), item.how));
            out.push('\n');
        }
        println!("{}", out.trim_end());
        Ok(())
    }

    pub fn print_error(&self, msg: &str) {
        if self.json {
            let out = serde_json::json!({
                "status": "error",
                "code": "error",
                "message": msg,
            });
            println!("{out}");
            return;
        }
        eprintln!("{} {msg}", self.style("error", "31;1"));
    }

    pub fn print_warning(&self, msg: &str) {
        if self.json {
            let out = serde_json::json!({
                "status": "warning",
                "code": "warning",
                "message": msg,
            });
            eprintln!("{out}");
            return;
        }
        eprintln!("{} {msg}", self.style("warn", "33;1"));
    }

    fn style(&self, text: &str, ansi: &str) -> String {
        if self.no_color {
            return text.to_string();
        }
        format!("\x1b[{ansi}m{text}\x1b[0m")
    }
}

fn render_value(value: &Value, indent: usize) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Array(items) => render_array(items, indent),
        Value::Object(map) => render_object(map, indent),
    }
}

fn render_array(items: &[Value], indent: usize) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }

    let pad = " ".repeat(indent);
    let mut out = String::new();
    for item in items {
        match item {
            Value::Object(_) | Value::Array(_) => {
                out.push_str(&format!("{pad}-\n{}\n", render_value(item, indent + 2)));
            }
            _ => out.push_str(&format!("{pad}- {}\n", render_value(item, indent + 2))),
        }
    }
    out.trim_end().to_string()
}

fn render_object(map: &serde_json::Map<String, Value>, indent: usize) -> String {
    if map.is_empty() {
        return "{}".to_string();
    }

    let pad = " ".repeat(indent);
    let mut out = String::new();
    for (key, value) in map {
        match value {
            Value::Object(_) | Value::Array(_) => {
                out.push_str(&format!(
                    "{pad}{key}:\n{}\n",
                    render_value(value, indent + 2)
                ));
            }
            _ => out.push_str(&format!(
                "{pad}{key}: {}\n",
                render_value(value, indent + 2)
            )),
        }
    }
    out.trim_end().to_string()
}
