use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

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
        ReportFormat::Sarif => pretty_json(&sarif(findings, source)),
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

fn sarif(findings: &[Finding], source: &Path) -> Value {
    let rules: BTreeMap<&str, &Finding> = findings
        .iter()
        .map(|finding| (finding.rule_id.as_str(), finding))
        .collect();
    let rules: Vec<_> = rules
        .into_iter()
        .map(|(rule_id, finding)| {
            json!({
                "id": rule_id,
                "name": rule_id,
                "shortDescription": { "text": finding.message },
                "defaultConfiguration": { "level": sarif_level(finding.severity) }
            })
        })
        .collect();
    let source = source.to_string_lossy();
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

    json!({
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
    })
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
