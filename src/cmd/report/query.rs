use super::*;

pub(super) fn query_value(root: &serde_json::Value, expr: &str) -> FozzyResult<serde_json::Value> {
    let expr = expr.trim();
    if expr == "." || expr == "$" {
        return Ok(root.clone());
    }
    let normalized = apply_query_aliases(&normalize_query_expr(expr)?);
    let tokens = parse_expr(&normalized)?;
    let mut current: Vec<&serde_json::Value> = vec![root];
    for token in tokens {
        let mut next = Vec::new();
        match token {
            QueryToken::Field(name) => {
                for v in &current {
                    if let Some(arr) = v.as_array()
                        && let Ok(idx) = name.parse::<usize>()
                        && let Some(item) = arr.get(idx)
                    {
                        next.push(item);
                        continue;
                    }
                    if let Some(field) = v.get(&name) {
                        next.push(field);
                    }
                }
            }
            QueryToken::Index(idx) => {
                for v in &current {
                    if let Some(item) = v.get(idx) {
                        next.push(item);
                    }
                }
            }
            QueryToken::AllIndices => {
                for v in &current {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            next.push(item);
                        }
                    }
                }
            }
        }
        current = next;
    }

    if current.is_empty() {
        let suggestions = suggest_query_paths(root, &normalized, 4);
        let suggestion_text = if suggestions.is_empty() {
            String::new()
        } else {
            format!("; did you mean {}", suggestions.join(", "))
        };
        return Err(FozzyError::Report(format!(
            "query matched no values for expression {expr:?}{suggestion_text}"
        )));
    }
    if current.len() == 1 {
        return Ok(current[0].clone());
    }
    Ok(serde_json::Value::Array(
        current.into_iter().cloned().collect(),
    ))
}

pub(super) fn list_query_paths(root: &serde_json::Value) -> Vec<String> {
    fn visit(
        value: &serde_json::Value,
        path: String,
        out: &mut std::collections::BTreeSet<String>,
    ) {
        out.insert(path.clone());
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let next = if path == "." {
                        format!(".{k}")
                    } else {
                        format!("{path}.{k}")
                    };
                    visit(v, next, out);
                }
            }
            serde_json::Value::Array(arr) => {
                out.insert(format!("{path}[]"));
                if let Some(first) = arr.first() {
                    visit(first, format!("{path}[0]"), out);
                }
            }
            _ => {}
        }
    }

    let mut out = std::collections::BTreeSet::new();
    visit(root, ".".to_string(), &mut out);
    out.into_iter()
        .map(|p| {
            if p == "." {
                ".".to_string()
            } else {
                p.trim_start_matches('.').to_string()
            }
        })
        .collect()
}

fn suggest_query_paths(
    root: &serde_json::Value,
    normalized_expr: &str,
    limit: usize,
) -> Vec<String> {
    let paths = list_query_paths(root);
    let needle = normalized_expr.trim_start_matches('.');
    let needle_lc = needle.to_ascii_lowercase();
    if needle.is_empty() {
        return paths.into_iter().take(limit).collect();
    }

    let mut exact_prefix: Vec<String> = paths
        .iter()
        .filter(|p| p.to_ascii_lowercase().starts_with(&needle_lc))
        .cloned()
        .collect();
    if exact_prefix.is_empty() {
        let tail_lc = needle_lc
            .rsplit('.')
            .next()
            .unwrap_or(&needle_lc)
            .to_string();
        exact_prefix = paths
            .iter()
            .filter(|p| {
                let p_lc = p.to_ascii_lowercase();
                p_lc.ends_with(&tail_lc) || p_lc.contains(&needle_lc)
            })
            .cloned()
            .collect();
    }
    exact_prefix.sort();
    exact_prefix.dedup();
    exact_prefix.into_iter().take(limit).collect()
}

fn apply_query_aliases(expr: &str) -> String {
    const ALIASES: &[(&str, &str)] = &[
        (".runId", ".identity.runId"),
        (".seed", ".identity.seed"),
        (".tracePath", ".identity.tracePath"),
        (".reportPath", ".identity.reportPath"),
        (".artifactsDir", ".identity.artifactsDir"),
    ];
    for (from, to) in ALIASES {
        if expr == *from {
            return (*to).to_string();
        }
        if let Some(rest) = expr.strip_prefix(from)
            && (rest.starts_with('.') || rest.starts_with('['))
        {
            return format!("{to}{rest}");
        }
    }
    expr.to_string()
}

fn normalize_query_expr(expr: &str) -> FozzyResult<String> {
    if expr.is_empty() {
        return Err(FozzyError::Report(
            "empty report path expression; examples: '.', '.identity.runId', 'findings[0].title', '.findings[].title'"
                .to_string(),
        ));
    }

    if let Some(rest) = expr.strip_prefix("$.") {
        return Ok(format!(".{rest}"));
    }
    if let Some(rest) = expr.strip_prefix('$') {
        if rest.starts_with('[') {
            return Ok(format!(".{rest}"));
        }
        return Err(FozzyError::Report(format!(
            "unsupported report path expression {expr:?}; supported path subset examples: '.', '.a.b', 'a.b', '.arr[0]', '.arr[].field'"
        )));
    }
    if expr.starts_with('.') {
        return Ok(expr.to_string());
    }
    if expr.starts_with('[')
        || expr
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return Ok(format!(".{expr}"));
    }
    Err(FozzyError::Report(format!(
        "unsupported report path expression {expr:?}; supported path subset examples: '.', '.a.b', 'a.b', '.arr[0]', '.arr[].field'"
    )))
}

#[derive(Debug, Clone)]
enum QueryToken {
    Field(String),
    Index(usize),
    AllIndices,
}

fn parse_expr(expr: &str) -> FozzyResult<Vec<QueryToken>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 1usize;

    while i < chars.len() {
        if chars[i] == '.' {
            i += 1;
            continue;
        }
        if chars[i] == '[' {
            i += 1;
            if i < chars.len() && chars[i] == ']' {
                i += 1;
                tokens.push(QueryToken::AllIndices);
                continue;
            }
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i >= chars.len() || chars[i] != ']' || start == i {
                return Err(FozzyError::Report(format!(
                    "invalid index expression in {expr:?}"
                )));
            }
            let idx_str: String = chars[start..i].iter().collect();
            i += 1;
            let idx: usize = idx_str
                .parse()
                .map_err(|_| FozzyError::Report(format!("invalid index {idx_str:?}")))?;
            tokens.push(QueryToken::Index(idx));
            continue;
        }

        let start = i;
        while i < chars.len() && chars[i] != '.' && chars[i] != '[' {
            i += 1;
        }
        let field: String = chars[start..i].iter().collect();
        if field.is_empty() {
            return Err(FozzyError::Report(format!(
                "invalid field expression in {expr:?}"
            )));
        }
        tokens.push(QueryToken::Field(field));
    }

    Ok(tokens)
}
