use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Newtype wrappers to prevent mixing IDs at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId(pub String);

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectPath(pub PathBuf);

impl ProjectPath {
    /// Decode the `-`-encoded directory name back to a real filesystem path.
    ///
    /// Claude Code encodes project paths by replacing `/` with `-`:
    /// - Leading `-` becomes `/`
    /// - Remaining `-` become `/`
    ///
    /// If the path does not start with `-`, it is returned unchanged.
    ///
    /// **Limitation:** This decoding is lossy. Dashes that are part of real
    /// directory names (e.g., `fred-drake`, `github.com`) are
    /// indistinguishable from path separators. For example,
    /// `-Users-fred-drake-project` decodes to `/Users/fred/drake/project`
    /// instead of the correct `/Users/fred-drake/project`. The output is
    /// an approximation useful for display only. Consider using the raw
    /// encoded name (via the `Display` trait) when exact identification
    /// is needed.
    pub fn decoded_path(&self) -> PathBuf {
        let s = self.0.to_string_lossy();
        if let Some(rest) = s.strip_prefix('-') {
            let decoded = format!("/{}", rest.replace('-', "/"));
            PathBuf::from(decoded)
        } else {
            self.0.clone()
        }
    }

    /// Extract the last component of the decoded path for display.
    ///
    /// Uses `decoded_path()` to convert the encoded name back to a path,
    /// then extracts the file name. Note: since decoding is lossy (dashes
    /// in real directory names are indistinguishable from path separators),
    /// the result is an approximation.
    ///
    /// - `-Users-fdrake` → `"fdrake"`
    /// - `-` (root) → `"/"`
    /// - Empty string → `""`
    /// - No leading dash (e.g. `"my-project"`) → return full string
    pub fn display_name(&self) -> String {
        let s = self.0.to_string_lossy();
        if s.is_empty() {
            return String::new();
        }
        if !s.starts_with('-') {
            return s.to_string();
        }
        let decoded = self.decoded_path();
        decoded
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string())
    }

    /// Check if this project path matches a given CWD.
    ///
    /// Claude Code encodes project paths by replacing `/` and `.` with `-`,
    /// so we encode the CWD the same way and compare.
    pub fn matches_cwd(&self, cwd: &Path) -> bool {
        let s = cwd.to_string_lossy();
        let normalized = s.trim_end_matches('/');
        if normalized.is_empty() {
            // Root "/" normalizes to "", encodes to "-"
            return self.0.to_string_lossy() == "-";
        }
        let encoded = normalized.replace(['/', '.'], "-");
        self.0.to_string_lossy() == encoded
    }
}

impl std::fmt::Display for ProjectPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

/// A parsed session with all its turns.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub project: ProjectPath,
    pub file_path: PathBuf,
    pub version: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    pub last_prompt: Option<String>,
    pub turns: Vec<Turn>,
    pub token_totals: TokenUsage,
    pub parse_warnings: Vec<ParseWarning>,
}

/// A turn is one user message + one assistant response.
#[derive(Debug, Clone)]
pub struct Turn {
    pub index: usize,
    pub user_message: UserMessage,
    pub assistant_response: Option<AssistantResponse>,
    pub duration: Option<std::time::Duration>,
    pub is_complete: bool,
    pub events: Vec<SessionEvent>,
}

#[derive(Debug, Clone)]
pub struct UserMessage {
    pub id: MessageId,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub content: UserContent,
}

#[derive(Debug, Clone)]
pub enum UserContent {
    Text(String),
    ToolResults(Vec<ToolResult>),
    Mixed {
        text: String,
        tool_results: Vec<ToolResult>,
    },
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub struct AssistantResponse {
    pub id: MessageId,
    pub model: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub content_blocks: Vec<ContentBlock>,
    pub usage: TokenUsage,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(String),
    Thinking { text: String },
    ToolUse(ToolCall),
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: ToolName,
    pub input: serde_json::Value,
    /// Populated later by correlating with tool_result.
    pub result: Option<ToolResult>,
}

/// Strongly typed tool names for pattern matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolName {
    Read,
    Edit,
    Write,
    Bash,
    Glob,
    Grep,
    WebSearch,
    WebFetch,
    Agent,
    TodoRead,
    TodoWrite,
    Other(String),
}

impl std::str::FromStr for ToolName {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "Read" => Self::Read,
            "Edit" => Self::Edit,
            "Write" => Self::Write,
            "Bash" => Self::Bash,
            "Glob" => Self::Glob,
            "Grep" => Self::Grep,
            "WebSearch" => Self::WebSearch,
            "WebFetch" => Self::WebFetch,
            "Agent" => Self::Agent,
            "TodoRead" => Self::TodoRead,
            "TodoWrite" => Self::TodoWrite,
            other => Self::Other(other.to_string()),
        })
    }
}

impl ToolName {
    /// Parse a tool name string into a ToolName variant.
    pub fn parse(s: &str) -> Self {
        s.parse().unwrap()
    }
}

impl std::fmt::Display for ToolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "Read"),
            Self::Edit => write!(f, "Edit"),
            Self::Write => write!(f, "Write"),
            Self::Bash => write!(f, "Bash"),
            Self::Glob => write!(f, "Glob"),
            Self::Grep => write!(f, "Grep"),
            Self::WebSearch => write!(f, "WebSearch"),
            Self::WebFetch => write!(f, "WebFetch"),
            Self::Agent => write!(f, "Agent"),
            Self::TodoRead => write!(f, "TodoRead"),
            Self::TodoWrite => write!(f, "TodoWrite"),
            Self::Other(name) => write!(f, "{name}"),
        }
    }
}

/// Token usage with cache breakdown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
}

impl TokenUsage {
    pub fn add(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_creation_tokens += other.cache_creation_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
    }

    /// Returns the sum of all token categories.
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens
    }
}

/// Format a token count for human-readable display.
///
/// - 0 -> "0"
/// - < 10,000 -> "1,234" (comma-separated)
/// - >= 10,000 -> "12.3k"
/// - >= 1,000,000 -> "1.2M"
pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        let whole = n / 1_000_000;
        let frac = (n % 1_000_000) / 100_000; // first decimal digit
        if frac == 0 {
            format!("{whole}M")
        } else {
            format!("{whole}.{frac}M")
        }
    } else if n >= 10_000 {
        let whole = n / 1_000;
        let frac = (n % 1_000) / 100; // first decimal digit
        if frac == 0 {
            format!("{whole}k")
        } else {
            format!("{whole}.{frac}k")
        }
    } else if n >= 1_000 {
        format!("{},{:03}", n / 1000, n % 1000)
    } else {
        n.to_string()
    }
}

/// The 7 attribution categories for context window analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenCategory {
    SystemPrompt,
    UserMessage,
    AssistantText,
    Thinking,
    ToolInput,
    ToolOutput,
    CacheRead,
}

impl std::fmt::Display for TokenCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SystemPrompt => write!(f, "System Prompt"),
            Self::UserMessage => write!(f, "User Message"),
            Self::AssistantText => write!(f, "Assistant Text"),
            Self::Thinking => write!(f, "Thinking"),
            Self::ToolInput => write!(f, "Tool Input"),
            Self::ToolOutput => write!(f, "Tool Output"),
            Self::CacheRead => write!(f, "Cache Read"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenAttribution {
    pub by_category: HashMap<TokenCategory, u64>,
    pub total: u64,
}

/// Progress and system events.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    HookProgress {
        hook_name: String,
        command: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TurnDuration {
        duration_ms: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    AgentSpawn {
        agent_id: String,
        agent_type: String,
        prompt: String,
        parent_tool_use_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    CompactionDetected {
        turn_index: usize,
        tokens_before: u64,
        tokens_after: u64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    QueueOperation {
        operation: String,
        content: Option<String>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Warnings generated during parsing.
#[derive(Debug, Clone)]
pub enum ParseWarning {
    MalformedLine { line: usize, reason: String },
    OrphanedRecord { uuid: String, record_type: String },
    MismatchedToolResult { tool_use_id: String },
    SkippedSidechain { uuid: String },
}

/// Summary for project list display (metadata-only, no file reading).
#[derive(Debug, Clone)]
pub struct ProjectSummary {
    pub path: ProjectPath,
    pub display_name: String,
    pub session_count: usize,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

/// Format a `DateTime<Utc>` as human-readable relative time.
///
/// Returns strings like "just now", "5 min ago", "2 hours ago", "3 days ago",
/// "2 weeks ago", "1 month ago", "1 year ago".
pub fn format_relative_time(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(dt);
    let secs = duration.num_seconds();

    if secs < 0 {
        return "just now".to_string();
    }

    if secs < 60 {
        return "just now".to_string();
    }

    let minutes = secs / 60;
    if minutes < 60 {
        if minutes == 1 {
            return "1 minute ago".to_string();
        }
        return format!("{minutes} minutes ago");
    }

    let hours = minutes / 60;
    if hours < 24 {
        if hours == 1 {
            return "1 hour ago".to_string();
        }
        return format!("{hours} hours ago");
    }

    let days = hours / 24;
    if days < 7 {
        if days == 1 {
            return "1 day ago".to_string();
        }
        return format!("{days} days ago");
    }

    if days < 30 {
        let weeks = days / 7;
        if weeks == 1 {
            return "1 week ago".to_string();
        }
        return format!("{weeks} weeks ago");
    }

    if days < 365 {
        let months = days / 30;
        if months == 1 {
            return "1 month ago".to_string();
        }
        return format!("{months} months ago");
    }

    let years = days / 365;
    if years == 1 {
        return "1 year ago".to_string();
    }
    format!("{years} years ago")
}

/// Summary for session list display (avoids loading full session).
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: SessionId,
    pub project: ProjectPath,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub last_prompt: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    pub turn_count: usize,
    pub total_tokens: TokenUsage,
    pub git_branch: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // --- ProjectPath::display_name tests ---

    #[rstest]
    #[case("-Users-fdrake-Source-github.com-fred-drake-claude-seer", "seer")]
    #[case("-Users-fdrake", "fdrake")]
    #[case("-", "/")]
    #[case("my-project", "my-project")]
    fn project_path_display_name(#[case] input: &str, #[case] expected: &str) {
        let p = ProjectPath(PathBuf::from(input));
        assert_eq!(p.display_name(), expected);
    }

    #[test]
    fn project_path_display_name_empty() {
        let p = ProjectPath(PathBuf::from(""));
        assert_eq!(p.display_name(), "");
    }

    // --- ProjectPath::matches_cwd tests ---

    #[test]
    fn project_path_matches_cwd_exact() {
        let p = ProjectPath(PathBuf::from("-Users-fdrake-Source-project"));
        assert!(p.matches_cwd(Path::new("/Users/fdrake/Source/project")));
    }

    #[test]
    fn project_path_matches_cwd_no_match() {
        let p = ProjectPath(PathBuf::from("-Users-fdrake-Source-project"));
        assert!(!p.matches_cwd(Path::new("/Users/fdrake/Source/other")));
    }

    #[test]
    fn project_path_matches_cwd_trailing_slash() {
        let p = ProjectPath(PathBuf::from("-Users-fdrake-Source-project"));
        assert!(p.matches_cwd(Path::new("/Users/fdrake/Source/project/")));
    }

    #[test]
    fn project_path_matches_cwd_root() {
        let p = ProjectPath(PathBuf::from("-"));
        assert!(p.matches_cwd(Path::new("/")));
    }

    #[test]
    fn project_path_matches_cwd_with_dots_in_path() {
        // Claude Code encodes both `/` and `.` as `-`
        let p = ProjectPath(PathBuf::from(
            "-Volumes-External-Users-fdrake-Source-github-com-fred-drake-claude-seer",
        ));
        assert!(p.matches_cwd(Path::new(
            "/Volumes/External/Users/fdrake/Source/github.com/fred-drake/claude-seer"
        )));
    }

    // --- format_relative_time tests ---

    #[rstest]
    #[case(0, "just now")]
    #[case(30, "just now")]
    #[case(59, "just now")]
    #[case(60, "1 minute ago")]
    #[case(300, "5 minutes ago")]
    #[case(3599, "59 minutes ago")]
    #[case(3600, "1 hour ago")]
    #[case(7200, "2 hours ago")]
    #[case(86399, "23 hours ago")]
    #[case(86400, "1 day ago")]
    #[case(172800, "2 days ago")]
    #[case(604800, "1 week ago")]
    #[case(1209600, "2 weeks ago")]
    #[case(2592000, "1 month ago")]
    #[case(5184000, "2 months ago")]
    #[case(31536000, "1 year ago")]
    #[case(63072000, "2 years ago")]
    fn format_relative_time_cases(#[case] secs_ago: i64, #[case] expected: &str) {
        let dt = chrono::Utc::now() - chrono::Duration::seconds(secs_ago);
        assert_eq!(format_relative_time(dt), expected);
    }

    #[test]
    fn format_relative_time_future_is_just_now() {
        let future = chrono::Utc::now() + chrono::Duration::seconds(600);
        assert_eq!(format_relative_time(future), "just now");
    }

    #[test]
    fn session_id_equality() {
        let a = SessionId("sess-001".to_string());
        let b = SessionId("sess-001".to_string());
        let c = SessionId("sess-002".to_string());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[rstest]
    #[case("Read", ToolName::Read)]
    #[case("Edit", ToolName::Edit)]
    #[case("Write", ToolName::Write)]
    #[case("Bash", ToolName::Bash)]
    #[case("Glob", ToolName::Glob)]
    #[case("Grep", ToolName::Grep)]
    #[case("WebSearch", ToolName::WebSearch)]
    #[case("WebFetch", ToolName::WebFetch)]
    #[case("Agent", ToolName::Agent)]
    #[case("TodoRead", ToolName::TodoRead)]
    #[case("TodoWrite", ToolName::TodoWrite)]
    #[case("CustomTool", ToolName::Other("CustomTool".to_string()))]
    fn tool_name_from_string(#[case] input: &str, #[case] expected: ToolName) {
        assert_eq!(ToolName::parse(input), expected);
    }

    #[test]
    fn token_usage_default_is_zero() {
        let usage = TokenUsage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cache_creation_tokens, 0);
        assert_eq!(usage.cache_read_tokens, 0);
    }

    #[test]
    fn token_usage_add_accumulates() {
        let mut total = TokenUsage::default();
        let a = TokenUsage {
            input_tokens: 100,
            output_tokens: 25,
            cache_creation_tokens: 50,
            cache_read_tokens: 10,
        };
        let b = TokenUsage {
            input_tokens: 200,
            output_tokens: 30,
            cache_creation_tokens: 0,
            cache_read_tokens: 50,
        };
        total.add(&a);
        total.add(&b);
        assert_eq!(total.input_tokens, 300);
        assert_eq!(total.output_tokens, 55);
        assert_eq!(total.cache_creation_tokens, 50);
        assert_eq!(total.cache_read_tokens, 60);
    }

    #[test]
    fn project_path_wraps_pathbuf() {
        let p = ProjectPath(PathBuf::from("/home/user/project"));
        assert_eq!(p.0, PathBuf::from("/home/user/project"));
    }

    #[test]
    fn project_path_decoded_path_converts_dashes_to_slashes() {
        // Leading dash becomes /, remaining dashes become /
        let p = ProjectPath(PathBuf::from("-home-user-project"));
        assert_eq!(p.decoded_path(), PathBuf::from("/home/user/project"));
    }

    #[test]
    fn project_path_decoded_path_single_dash_is_root() {
        let p = ProjectPath(PathBuf::from("-"));
        assert_eq!(p.decoded_path(), PathBuf::from("/"));
    }

    #[test]
    fn project_path_decoded_path_no_leading_dash_unchanged() {
        let p = ProjectPath(PathBuf::from("relative-path-here"));
        assert_eq!(p.decoded_path(), PathBuf::from("relative-path-here"));
    }

    #[test]
    fn message_id_equality() {
        let a = MessageId("msg-001".to_string());
        let b = MessageId("msg-001".to_string());
        assert_eq!(a, b);
    }

    #[test]
    fn session_id_display() {
        let id = SessionId("sess-001".to_string());
        assert_eq!(format!("{}", id), "sess-001");
    }

    #[test]
    fn tool_name_display() {
        assert_eq!(format!("{}", ToolName::Read), "Read");
        assert_eq!(format!("{}", ToolName::Bash), "Bash");
        assert_eq!(
            format!("{}", ToolName::Other("Custom".to_string())),
            "Custom"
        );
    }

    #[test]
    fn message_id_display() {
        let id = MessageId("msg-001".to_string());
        assert_eq!(format!("{}", id), "msg-001");
    }

    #[test]
    fn project_path_display() {
        let p = ProjectPath(PathBuf::from("/home/user/project"));
        assert_eq!(format!("{}", p), "/home/user/project");
    }

    #[test]
    fn token_category_display() {
        assert_eq!(format!("{}", TokenCategory::SystemPrompt), "System Prompt");
        assert_eq!(format!("{}", TokenCategory::UserMessage), "User Message");
        assert_eq!(format!("{}", TokenCategory::CacheRead), "Cache Read");
    }

    #[test]
    fn token_usage_total() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 25,
            cache_read_tokens: 10,
        };
        assert_eq!(usage.total(), 185);
    }

    #[test]
    fn token_usage_total_default_is_zero() {
        assert_eq!(TokenUsage::default().total(), 0);
    }

    #[rstest]
    #[case(0, "0")]
    #[case(1, "1")]
    #[case(999, "999")]
    #[case(1_000, "1,000")]
    #[case(1_234, "1,234")]
    #[case(9_999, "9,999")]
    #[case(10_000, "10k")]
    #[case(12_345, "12.3k")]
    #[case(12_500, "12.5k")]
    #[case(99_900, "99.9k")]
    #[case(100_000, "100k")]
    #[case(999_900, "999.9k")]
    #[case(1_000_000, "1M")]
    #[case(1_200_000, "1.2M")]
    #[case(1_500_000, "1.5M")]
    #[case(10_000_000, "10M")]
    fn format_tokens_cases(#[case] input: u64, #[case] expected: &str) {
        assert_eq!(format_tokens(input), expected);
    }

    #[test]
    fn project_path_ord() {
        let a = ProjectPath(PathBuf::from("/a/path"));
        let b = ProjectPath(PathBuf::from("/b/path"));
        assert!(a < b);
        let mut paths = vec![b.clone(), a.clone()];
        paths.sort();
        assert_eq!(paths, vec![a, b]);
    }
}
