use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};
use shieldspec_core::{Finding, Severity};

pub(crate) enum ReportFormat {
    Human,
    Json,
    Sarif,
}

#[derive(Serialize)]
struct JsonReport<'a> {
    passes: bool,
    findings: &'a [Finding],
}

pub(crate) fn render(
    passes: bool,
    findings: &[Finding],
    format: ReportFormat,
    source: &Path,
) -> Result<String, String> {
    match format {
        ReportFormat::Human => Ok(human(passes, findings)),
        ReportFormat::Json => pretty_json(&JsonReport { passes, findings }),
        ReportFormat::Sarif => pretty_json(&sarif(findings, source)?),
    }
}

fn human(passes: bool, findings: &[Finding]) -> String {
    let mut output = String::new();
    if passes {
        output.push_str("PASS: no policy-failing privacy findings\n");
        return output;
    }

    let noun = if findings.len() == 1 {
        "finding"
    } else {
        "findings"
    };
    writeln!(output, "FAIL: {} privacy {noun}", findings.len())
        .expect("writing to String cannot fail");
    for finding in findings {
        writeln!(
            output,
            "[{}] {} at step {} ({})",
            severity_name(finding.severity),
            finding.rule_id,
            finding.step_index,
            finding.fingerprint
        )
        .expect("writing to String cannot fail");
        writeln!(output, "  {}", finding.message).expect("writing to String cannot fail");
        output.push_str("  evidence:");
        for (key, value) in &finding.evidence {
            write!(output, " {key}={value}").expect("writing to String cannot fail");
        }
        output.push('\n');
    }
    output
}

fn sarif(findings: &[Finding], source: &Path) -> Result<Value, String> {
    let rule_ids: BTreeSet<&str> = findings
        .iter()
        .map(|finding| finding.rule_id.as_str())
        .collect();
    let rules: Vec<_> = rule_ids
        .into_iter()
        .map(|rule_id| {
            let metadata = rule_metadata(rule_id);
            json!({
                "id": rule_id,
                "name": rule_id,
                "shortDescription": { "text": metadata.description },
                "defaultConfiguration": { "level": sarif_level(metadata.default_severity) }
            })
        })
        .collect();
    let source = artifact_uri(source)?;
    let results: Vec<_> = findings
        .iter()
        .map(|finding| {
            json!({
                "ruleId": finding.rule_id,
                "level": sarif_level(finding.severity),
                "message": { "text": finding.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": source }
                    }
                }],
                "partialFingerprints": {
                    "shieldspecFingerprint": finding.fingerprint
                },
                "properties": {
                    "severity": severity_name(finding.severity),
                    "step_index": finding.step_index,
                    "evidence": finding.evidence,
                    "fingerprint": finding.fingerprint
                }
            })
        })
        .collect();

    Ok(json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "ShieldSpec",
                    "informationUri": "https://github.com/Ay-obami/ShieldSpec",
                    "semanticVersion": env!("CARGO_PKG_VERSION"),
                    "rules": rules
                }
            },
            "results": results
        }]
    }))
}

#[derive(Clone, Copy)]
struct RuleMetadata {
    description: &'static str,
    default_severity: Severity,
}

fn rule_metadata(rule_id: &str) -> RuleMetadata {
    match rule_id {
        "SS001_TRANSPARENT_RECIPIENT" => RuleMetadata {
            description: "Recipient output uses the transparent pool",
            default_severity: Severity::Warning,
        },
        "SS002_TRANSPARENT_FUNDING" => RuleMetadata {
            description: "Proposal spends transparent funds",
            default_severity: Severity::Warning,
        },
        "SS003_TRANSPARENT_CHANGE" => RuleMetadata {
            description: "Change output uses the transparent pool",
            default_severity: Severity::Error,
        },
        "SS004_UNKNOWN_ROUTE" => RuleMetadata {
            description: "Proposal route has an unknown pool",
            default_severity: Severity::Warning,
        },
        "SS005_PRIVACY_BOUNDARY" => RuleMetadata {
            description: "Value crosses the transparent/shielded privacy boundary",
            default_severity: Severity::Warning,
        },
        _ => RuleMetadata {
            description: "ShieldSpec policy finding",
            default_severity: Severity::Warning,
        },
    }
}

fn artifact_uri(source: &Path) -> Result<String, String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("could not determine current directory: {error}"))?;
    let source_root = env::var_os("SHIELDSPEC_SOURCE_ROOT");
    Ok(artifact_uri_with_context(
        source,
        &current_dir,
        source_root.as_deref().map(Path::new),
    ))
}

fn artifact_uri_with_context(
    source: &Path,
    current_dir: &Path,
    source_root: Option<&Path>,
) -> String {
    let current_dir = normalize_path(current_dir);
    let absolute_source = if source.is_absolute() {
        normalize_path(source)
    } else {
        normalize_path(&current_dir.join(source))
    };

    if let Some(source_root) = source_root.filter(|root| root.is_absolute()) {
        if let Ok(relative) = absolute_source.strip_prefix(normalize_path(source_root)) {
            let relative = encode_uri_path(relative);
            return if relative.is_empty() {
                ".".to_owned()
            } else {
                relative
            };
        }
    }

    if let Ok(relative) = absolute_source.strip_prefix(&current_dir) {
        let relative = encode_uri_path(relative);
        return if relative.is_empty() {
            ".".to_owned()
        } else {
            relative
        };
    }

    let encoded = encode_uri_path(&absolute_source);
    if encoded.starts_with("//") {
        format!("file:{encoded}")
    } else if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn encode_uri_path(path: &Path) -> String {
    let path = path
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            encoded.push(char::from(byte));
        } else {
            write!(encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

fn pretty_json(value: &impl Serialize) -> Result<String, String> {
    let mut output = serde_json::to_string_pretty(value)
        .map_err(|error| format!("could not serialize report: {error}"))?;
    output.push('\n');
    Ok(output)
}

const fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

const fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "note",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::artifact_uri_with_context;
    use std::path::Path;

    #[test]
    fn artifact_uri_is_repository_relative_under_source_root() {
        assert_eq!(
            artifact_uri_with_context(
                Path::new("/workspace/repo/fixtures/a #%.json"),
                Path::new("/opt/shieldspec"),
                Some(Path::new("/workspace/repo")),
            ),
            "fixtures/a%20%23%25.json"
        );
    }

    #[test]
    fn artifact_uri_outside_source_root_keeps_file_uri_fallback() {
        assert_eq!(
            artifact_uri_with_context(
                Path::new("/workspace/external/a #%.json"),
                Path::new("/opt/shieldspec"),
                Some(Path::new("/workspace/repo")),
            ),
            "file:///workspace/external/a%20%23%25.json"
        );
    }
}
