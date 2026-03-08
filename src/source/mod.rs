pub mod error;
pub mod filesystem;

use crate::data::model::{Session, SessionId, SessionSummary};
use crate::source::error::SourceError;

/// Abstraction over where session data comes from.
/// Tests provide an in-memory implementation, production uses filesystem.
pub trait DataSource: Send + Sync {
    /// List all available session summaries.
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SourceError>;

    /// Load and fully parse a single session.
    fn load_session(&self, id: &SessionId) -> Result<Session, SourceError>;
}
