pub mod error;
pub mod filesystem;

use crate::data::model::{ProjectPath, ProjectSummary, Session, SessionId, SessionSummary};
use crate::source::error::SourceError;

/// Abstraction over where session data comes from.
/// Tests provide an in-memory implementation, production uses filesystem.
pub trait DataSource: Send + Sync {
    /// List all projects with metadata (session count, last activity).
    /// Uses only filesystem metadata — no file reading for performance.
    fn list_projects(&self) -> Result<Vec<ProjectSummary>, SourceError>;

    /// List session summaries scoped to a single project.
    fn list_sessions_for_project(
        &self,
        project: &ProjectPath,
    ) -> Result<Vec<SessionSummary>, SourceError>;

    /// List all available session summaries.
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SourceError>;

    /// Load and fully parse a single session.
    fn load_session(&self, id: &SessionId) -> Result<Session, SourceError>;
}
