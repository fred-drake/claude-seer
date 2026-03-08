use crate::data::error::DataError;
use crate::data::model::SessionId;

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("projects directory not found: {}", .0.display())]
    DirectoryNotFound(std::path::PathBuf),

    #[error("failed to read session file")]
    IoError(#[from] std::io::Error),

    #[error("data parsing failed")]
    DataError(#[from] DataError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::error::DataError;
    use crate::data::model::SessionId;

    #[test]
    fn session_not_found_displays_id() {
        let err = SourceError::SessionNotFound(SessionId("sess-123".to_string()));
        assert_eq!(err.to_string(), "session not found: sess-123");
    }

    #[test]
    fn io_error_converts_from_std_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: SourceError = io_err.into();
        assert_eq!(err.to_string(), "failed to read session file");
    }

    #[test]
    fn directory_not_found_displays_path() {
        let err =
            SourceError::DirectoryNotFound(std::path::PathBuf::from("/home/user/.claude/projects"));
        assert_eq!(
            err.to_string(),
            "projects directory not found: /home/user/.claude/projects"
        );
    }

    #[test]
    fn data_error_converts_from_data_error() {
        let data_err = DataError::ParseError {
            line: 1,
            reason: "bad json".to_string(),
        };
        let err: SourceError = data_err.into();
        assert_eq!(err.to_string(), "data parsing failed");
    }
}
