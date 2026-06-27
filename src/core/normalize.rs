//! Stream Normalizer: RTK emits plain text lines; Cotrex classifies each by severity so the
//! orchestrator can count errors and decide whether a failure is worth an LLM insight. The line
//! text itself is passed through verbatim — wrapping every line in JSON would cost the agent more
//! tokens than the raw command output it's meant to compress.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// One normalized line of RTK output: the verbatim text plus its classified severity.
#[derive(Debug, Clone, PartialEq)]
pub struct LineEvent {
    pub line: String,
    pub severity: Severity,
}

/// Classify a line by keyword. Case-insensitive substring match — deliberately blunt.
/// ponytail: keyword heuristic; replace with per-tool parsers only if severity matters downstream.
pub fn classify(line: &str) -> Severity {
    let l = line.to_ascii_lowercase();
    if l.contains("error") || l.contains("failed") || l.contains("panic") || l.contains("fatal") {
        Severity::Error
    } else if l.contains("warning") || l.contains("warn") {
        Severity::Warning
    } else {
        Severity::Info
    }
}

pub fn normalize(line: String) -> LineEvent {
    let severity = classify(&line);
    LineEvent { line, severity }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_classifier() {
        assert_eq!(classify("error: cannot find crate serde"), Severity::Error);
        assert_eq!(classify("test result: FAILED"), Severity::Error);
        assert_eq!(classify("warning: unused import"), Severity::Warning);
        assert_eq!(classify("Compiling cotrex v0.1.0"), Severity::Info);
    }

    #[test]
    fn normalize_keeps_line_verbatim() {
        let e = normalize("panic at the disco".into());
        assert_eq!(e.line, "panic at the disco");
        assert_eq!(e.severity, Severity::Error);
    }
}
