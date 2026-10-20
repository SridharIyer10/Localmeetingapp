//! JSON-array helpers for multi-value text fields.
//!
//! Question text routinely contains commas ("Approved by Q3, pending CFO").
//! Splitting on `,` is a silent data-corruption bug, so multi-value fields are
//! stored as JSON arrays and parsed on read.

/// Parse a JSON array of strings; invalid or empty input yields an empty vector.
pub fn json_string_array(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

/// Serialize a slice of strings into a JSON array.
pub fn to_json_array(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let values = vec![
            "Approved by Q3, pending CFO".to_string(),
            "second".to_string(),
        ];
        let json = to_json_array(&values);
        assert_eq!(json_string_array(&json), values);
    }

    #[test]
    fn comma_containing_value_is_one_item() {
        let parsed = json_string_array(r#"["Approved by Q3, pending CFO"]"#);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], "Approved by Q3, pending CFO");
    }

    #[test]
    fn invalid_json_is_empty_not_panic() {
        assert!(json_string_array("not json").is_empty());
        assert!(json_string_array("").is_empty());
    }
}
