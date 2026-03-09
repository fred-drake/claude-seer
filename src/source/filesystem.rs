use std::path::PathBuf;

use crate::data::model::{ProjectPath, ProjectSummary, Session, SessionId, SessionSummary};
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
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("skipping unreadable entry: {e}");
                    continue;
                }
            };
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            let project_path = ProjectPath(PathBuf::from(entry.file_name()));

            // Find .jsonl files in this project directory.
            let dir_entries = match std::fs::read_dir(&project_dir) {
                Ok(entries) => entries,
                Err(e) => {
                    tracing::warn!(
                        "skipping unreadable project dir {}: {e}",
                        project_dir.display()
                    );
                    continue;
                }
            };
            for file_entry in dir_entries {
                let file_entry = match file_entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
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

    /// Discover JSONL files within a specific project directory.
    fn discover_project_session_files(
        &self,
        project: &ProjectPath,
    ) -> Result<Vec<PathBuf>, SourceError> {
        let project_dir = self.projects_path.join(&project.0);
        if !project_dir.exists() {
            return Err(SourceError::DirectoryNotFound(project_dir));
        }

        let mut files = Vec::new();
        let dir_entries = std::fs::read_dir(&project_dir)?;
        for file_entry in dir_entries {
            let file_entry = file_entry?;
            let file_path = file_entry.path();
            if file_path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                files.push(file_path);
            }
        }
        Ok(files)
    }
}

impl DataSource for FilesystemSource {
    fn list_projects(&self) -> Result<Vec<ProjectSummary>, SourceError> {
        if !self.projects_path.exists() {
            return Err(SourceError::DirectoryNotFound(self.projects_path.clone()));
        }

        let mut projects = Vec::new();
        let entries = std::fs::read_dir(&self.projects_path)?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("skipping unreadable entry: {e}");
                    continue;
                }
            };

            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            let project_path = ProjectPath(PathBuf::from(entry.file_name()));

            // Count .jsonl files and find newest modification time (metadata only).
            let mut session_count = 0usize;
            let mut newest: Option<std::time::SystemTime> = None;

            let dir_entries = match std::fs::read_dir(&project_dir) {
                Ok(entries) => entries,
                Err(e) => {
                    tracing::warn!(
                        "skipping unreadable project dir {}: {e}",
                        project_dir.display()
                    );
                    continue;
                }
            };

            for file_entry in dir_entries {
                let file_entry = match file_entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let file_path = file_entry.path();
                if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }

                session_count += 1;

                if let Ok(meta) = std::fs::metadata(&file_path)
                    && let Ok(modified) = meta.modified()
                {
                    newest = Some(match newest {
                        Some(current) => current.max(modified),
                        None => modified,
                    });
                }
            }

            // Skip projects with 0 .jsonl files.
            if session_count == 0 {
                continue;
            }

            let last_activity = newest.map(chrono::DateTime::<chrono::Utc>::from);
            let display_name = project_path.display_name();

            projects.push(ProjectSummary {
                path: project_path,
                display_name,
                session_count,
                last_activity,
            });
        }

        Ok(projects)
    }

    fn list_sessions_for_project(
        &self,
        project: &ProjectPath,
    ) -> Result<Vec<SessionSummary>, SourceError> {
        let files = self.discover_project_session_files(project)?;
        let mut summaries = Vec::new();

        for file_path in &files {
            let file = std::fs::File::open(file_path)?;
            let file_size = file.metadata()?.len();
            let content = std::io::read_to_string(file)?;
            let id = Self::session_id_from_path(file_path);

            let scan = summary_scan(&content);
            let last_prompt = extract_last_prompt(&content);

            summaries.push(SessionSummary {
                id,
                project: project.clone(),
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

    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SourceError> {
        let files = self.discover_session_files()?;
        let mut summaries = Vec::new();

        for (file_path, project_path) in &files {
            // TODO: For large files, read only the first/last lines instead of the
            // entire file. This would significantly improve performance for sessions
            // with many turns.
            let file = std::fs::File::open(file_path)?;
            let file_size = file.metadata()?.len();
            let content = std::io::read_to_string(file)?;
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

    // --- list_projects tests ---

    #[test]
    fn list_projects_discovers_projects_with_sessions() {
        let source = FilesystemSource::new(test_claude_home());
        let projects = source.list_projects().unwrap();
        // my-project and other-project have .jsonl files; empty-project does not.
        assert_eq!(projects.len(), 2);
    }

    #[test]
    fn list_projects_skips_empty_project() {
        let source = FilesystemSource::new(test_claude_home());
        let projects = source.list_projects().unwrap();
        let names: Vec<&str> = projects.iter().map(|p| p.display_name.as_str()).collect();
        assert!(
            !names.contains(&"empty-project"),
            "Should skip empty-project, got: {:?}",
            names
        );
    }

    #[test]
    fn list_projects_has_correct_session_counts() {
        let source = FilesystemSource::new(test_claude_home());
        let projects = source.list_projects().unwrap();
        let my_proj = projects
            .iter()
            .find(|p| p.display_name == "my-project")
            .unwrap();
        assert_eq!(my_proj.session_count, 2);
        let other_proj = projects
            .iter()
            .find(|p| p.display_name == "other-project")
            .unwrap();
        assert_eq!(other_proj.session_count, 1);
    }

    #[test]
    fn list_projects_has_last_activity() {
        let source = FilesystemSource::new(test_claude_home());
        let projects = source.list_projects().unwrap();
        for project in &projects {
            assert!(
                project.last_activity.is_some(),
                "Project {} should have last_activity",
                project.display_name
            );
        }
    }

    #[test]
    fn list_projects_skips_non_directory_entries() {
        // stray-file.txt exists at the projects level; it should not cause errors
        // or appear as a project.
        let source = FilesystemSource::new(test_claude_home());
        let projects = source.list_projects().unwrap();
        let names: Vec<&str> = projects.iter().map(|p| p.display_name.as_str()).collect();
        assert!(
            !names.contains(&"stray-file.txt"),
            "Should skip non-directory entries, got: {:?}",
            names
        );
    }

    #[test]
    fn list_projects_error_for_nonexistent_path() {
        let source = FilesystemSource::new(PathBuf::from("/nonexistent/path"));
        let result = source.list_projects();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SourceError::DirectoryNotFound(_)
        ));
    }

    // --- list_sessions_for_project tests ---

    #[test]
    fn list_sessions_for_project_scoped_to_one_project() {
        let source = FilesystemSource::new(test_claude_home());
        let project = ProjectPath(PathBuf::from("my-project"));
        let sessions = source.list_sessions_for_project(&project).unwrap();
        assert_eq!(sessions.len(), 2);
        for s in &sessions {
            assert_eq!(s.project, project);
        }
    }

    #[test]
    fn list_sessions_for_project_nonexistent_returns_error() {
        let source = FilesystemSource::new(test_claude_home());
        let project = ProjectPath(PathBuf::from("nonexistent-project"));
        let result = source.list_sessions_for_project(&project);
        assert!(result.is_err());
    }

    #[test]
    fn list_sessions_for_empty_project_returns_empty() {
        let source = FilesystemSource::new(test_claude_home());
        let project = ProjectPath(PathBuf::from("empty-project"));
        let sessions = source.list_sessions_for_project(&project).unwrap();
        assert!(sessions.is_empty());
    }

    // --- existing tests ---

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
