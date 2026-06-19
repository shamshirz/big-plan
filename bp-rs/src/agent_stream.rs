//! Parse token usage from agent NDJSON (`stream-json`) stdout.

use serde_json::Value;

/// Token/model metrics extracted from the terminal `result` event in stream-json output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentUsageMetrics {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub model: Option<String>,
}

impl AgentUsageMetrics {
    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none() && self.output_tokens.is_none() && self.model.is_none()
    }

    /// Incorporate a single NDJSON line; keeps the latest `type: result` event seen.
    pub fn absorb_line(&mut self, line: &str) {
        if let Some(parsed) = parse_result_event(line) {
            if parsed.input_tokens.is_some() {
                self.input_tokens = parsed.input_tokens;
            }
            if parsed.output_tokens.is_some() {
                self.output_tokens = parsed.output_tokens;
            }
            if parsed.model.is_some() {
                self.model = parsed.model;
            }
        }
    }
}

/// Parse the terminal `result` line from Claude (`snake_case`) or Cursor (`camelCase`) stream-json.
pub fn parse_result_event(line: &str) -> Option<AgentUsageMetrics> {
    let line = line.trim();
    if line.is_empty() || !line.starts_with('{') {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(Value::as_str) != Some("result") {
        return None;
    }
    let usage = v.get("usage")?;
    Some(AgentUsageMetrics {
        input_tokens: json_i64(usage, &["input_tokens", "inputTokens"]),
        output_tokens: json_i64(usage, &["output_tokens", "outputTokens"]),
        model: v
            .get("model")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
    })
}

fn json_i64(obj: &Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        if let Some(n) = obj.get(*key).and_then(Value::as_i64) {
            return Some(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_snake_case_result() {
        let line = r#"{"type":"result","usage":{"input_tokens":1200,"output_tokens":340}}"#;
        let m = parse_result_event(line).unwrap();
        assert_eq!(m.input_tokens, Some(1200));
        assert_eq!(m.output_tokens, Some(340));
    }

    #[test]
    fn parses_cursor_camel_case_result() {
        let line = r#"{"type":"result","subtype":"success","usage":{"inputTokens":500,"outputTokens":42},"model":"composer-2.5"}"#;
        let m = parse_result_event(line).unwrap();
        assert_eq!(m.input_tokens, Some(500));
        assert_eq!(m.output_tokens, Some(42));
        assert_eq!(m.model.as_deref(), Some("composer-2.5"));
    }

    #[test]
    fn ignores_non_result_lines() {
        assert!(parse_result_event(r#"{"type":"assistant","message":{}}"#).is_none());
        assert!(parse_result_event("not json").is_none());
    }

    #[test]
    fn absorb_line_keeps_last_result() {
        let mut acc = AgentUsageMetrics::default();
        acc.absorb_line(r#"{"type":"result","usage":{"input_tokens":10,"output_tokens":1}}"#);
        acc.absorb_line(r#"{"type":"result","usage":{"input_tokens":99,"output_tokens":5}}"#);
        assert_eq!(acc.input_tokens, Some(99));
        assert_eq!(acc.output_tokens, Some(5));
    }
}
