#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("failed to parse JSONL at line {line}: {reason}")]
    ParseError { line: usize, reason: String },

    #[error("missing required field '{field}' in record type '{record_type}'")]
    MissingField { field: String, record_type: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_displays_line_and_reason() {
        let err = DataError::ParseError {
            line: 42,
            reason: "unexpected token".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to parse JSONL at line 42: unexpected token"
        );
    }

    #[test]
    fn missing_field_displays_field_and_record_type() {
        let err = DataError::MissingField {
            field: "uuid".to_string(),
            record_type: "user".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "missing required field 'uuid' in record type 'user'"
        );
    }
}
