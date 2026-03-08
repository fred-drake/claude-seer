use std::path::PathBuf;

use crate::data::model::{ProjectPath, Session, SessionId, SessionSummary};
use crate::data::session_loader::{extract_last_prompt, load_session_from_str, summary_scan};
use crate::source::DataSource;
use crate::source::error::SourceError;

/// Production DataSource implementation that reads from the filesystem.
pub struct FilesystemSource {
    /// Root path to the projects directory (e.g., ~/.claude/projects/).
    projects_path: PathBuf,
}

impl FilesystemSource {
    pub fn new(projects_path: PathBuf) -> Self {
        Self { projects_path }
    }

    /// Discover all JSONL session files under the projects directory.
    fn discover_session_files(&self) -> Result<Vec<(PathBuf, ProjectPath)>, SourceError> {
        let mut files = Vec::new();

        if !self.projects_path.exists() {
            return Err(SourceError::DirectoryNotFound(self.projects_path.clone()));
        }

        // Iterate project directories.
        let entries = std::fs::read_dir(&self.projects_path)?;
        for entry in entries {
            let entry = entry?;
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            let project_path = ProjectPath(PathBuf::from(entry.file_name()));

            // Find .jsonl files in this project directory.
            let dir_entries = std::fs::read_dir(&project_dir)?;
            for file_entry in dir_entries {
                let file_entry = file_entry?;
                let file_path = file_entry.path();
                if file_path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    files.push((file_path, project_path.clone()));
                }
            }
        }

        Ok(files)
    }

    /// Extract a session ID from a file path (filename without extension).
    fn session_id_from_path(path: &std::path::Path) -> SessionId {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        SessionId(stem.to_string())
    }
}

impl DataSource for FilesystemSource {
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SourceError> {
        let files = self.discover_session_files()?;
        let mut summaries = Vec::new();

        for (file_path, project_path) in &files {
            // TODO: For large files, read only the first/last lines instead of the
            // entire file. This would significantly improve performance for sessions
            // with many turns.
            let content = std::fs::read_to_string(file_path)?;
            let file_size = std::fs::metadata(file_path)?.len();
            let id = Self::session_id_from_path(file_path);

            let scan = summary_scan(&content);
            let last_prompt = extract_last_prompt(&content);

            summaries.push(SessionSummary {
                id,
                project: project_path.clone(),
                file_path: file_path.clone(),
                file_size,
                last_prompt,
                started_at: scan.started_at,
                last_activity: scan.last_activity,
                turn_count: scan.turn_count,
                total_tokens: scan.total_tokens,
                git_branch: scan.git_branch,
            });
        }

        Ok(summaries)
    }

    fn load_session(&self, id: &SessionId) -> Result<Session, SourceError> {
        // TODO: Cache discover_session_files() results to avoid re-scanning
        // the filesystem on every load_session call.
        let files = self.discover_session_files()?;
        let (file_path, project_path) = files
            .into_iter()
            .find(|(path, _)| Self::session_id_from_path(path) == *id)
            .ok_or_else(|| SourceError::SessionNotFound(id.clone()))?;

        let content = std::fs::read_to_string(&file_path)?;
        let session = load_session_from_str(&content, id.clone(), project_path, file_path)?;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_claude_home() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude_home/projects")
    }

    #[test]
    fn list_sessions_discovers_all_sessions() {
        let source = FilesystemSource::new(test_claude_home());
        let sessions = source.list_sessions().unwrap();
        // claude_home has 2 sessions in my-project and 1 in other-project.
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn list_sessions_returns_error_for_nonexistent_path() {
        let source = FilesystemSource::new(PathBuf::from("/nonexistent/path"));
        let result = source.list_sessions();
        assert!(result.is_err());
        match result.unwrap_err() {
            SourceError::DirectoryNotFound(path) => {
                assert_eq!(path, PathBuf::from("/nonexistent/path"));
            }
            other => panic!("expected DirectoryNotFound, got {:?}", other),
        }
    }

    #[test]
    fn load_session_by_id() {
        let source = FilesystemSource::new(test_claude_home());
        let session = source
            .load_session(&SessionId("session-1".to_string()))
            .unwrap();
        assert!(!session.turns.is_empty());
    }

    #[test]
    fn load_session_not_found() {
        let source = FilesystemSource::new(test_claude_home());
        let result = source.load_session(&SessionId("nonexistent".to_string()));
        assert!(result.is_err());
        match result.unwrap_err() {
            SourceError::SessionNotFound(id) => {
                assert_eq!(id, SessionId("nonexistent".to_string()))
            }
            other => panic!("expected SessionNotFound, got {:?}", other),
        }
    }

    #[test]
    fn list_sessions_has_metadata() {
        let source = FilesystemSource::new(test_claude_home());
        let sessions = source.list_sessions().unwrap();
        // All sessions should have timestamps.
        for session in &sessions {
            assert!(session.started_at.is_some());
            assert!(session.file_size > 0);
        }
    }

    #[test]
    fn session_id_from_path_normal() {
        let path = PathBuf::from("/some/dir/my-session.jsonl");
        let id = FilesystemSource::session_id_from_path(&path);
        assert_eq!(id, SessionId("my-session".to_string()));
    }

    #[test]
    fn session_id_from_path_no_stem_falls_back_to_unknown() {
        // A path with no file name component.
        let path = PathBuf::from("/");
        let id = FilesystemSource::session_id_from_path(&path);
        assert_eq!(id, SessionId("unknown".to_string()));
    }

    #[test]
    fn project_path_contains_directory_name_not_full_path() {
        let source = FilesystemSource::new(test_claude_home());
        let sessions = source.list_sessions().unwrap();
        // ProjectPath should contain just the directory name (e.g., "my-project"),
        // not the full filesystem path (e.g., "/path/to/projects/my-project").
        for session in &sessions {
            let path_str = session.project.0.to_string_lossy().to_string();
            // Should NOT contain path separators — it's just a directory name.
            assert!(
                !path_str.contains('/'),
                "ProjectPath should be directory name only, got: {}",
                path_str
            );
        }
    }

    #[test]
    fn list_sessions_has_last_prompt() {
        let source = FilesystemSource::new(test_claude_home());
        let sessions = source.list_sessions().unwrap();
        // At least some sessions should have a last prompt.
        let has_prompt = sessions.iter().any(|s| s.last_prompt.is_some());
        assert!(has_prompt);
    }
}
