// Claude Code version and usage/quota data.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::time::Duration;

/// Errors that can occur when fetching usage/version data.
#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("claude CLI not found or failed to execute: {0}")]
    VersionCommand(String),

    #[error("failed to read credentials: {0}")]
    CredentialRead(String),

    #[error("failed to parse credentials JSON: {0}")]
    CredentialParse(String),

    #[error("HTTP request failed: {0}")]
    HttpRequest(String),

    #[error("failed to parse usage response: {0}")]
    ResponseParse(String),
}

/// Usage utilization window (e.g., 5-hour or 7-day).
#[derive(Debug, Clone, Deserialize)]
pub struct UsageWindow {
    /// Utilization percentage (0.0 - 100.0).
    pub utilization: f64,
    /// When this window resets.
    pub resets_at: Option<DateTime<Utc>>,
}

/// Usage data from the Anthropic OAuth usage endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UsageData {
    #[serde(default)]
    pub five_hour: Option<UsageWindow>,
    #[serde(default)]
    pub seven_day: Option<UsageWindow>,
    #[serde(default)]
    pub seven_day_opus: Option<UsageWindow>,
}

/// Info displayed in the title bar.
#[derive(Debug, Clone)]
pub struct TitleBarInfo {
    pub claude_version: Option<String>,
    pub usage: Option<UsageData>,
}

impl TitleBarInfo {
    pub fn new() -> Self {
        Self {
            claude_version: None,
            usage: None,
        }
    }
}

impl Default for TitleBarInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Get Claude Code version by running `claude --version`.
pub fn fetch_claude_version() -> Result<String, UsageError> {
    let output = std::process::Command::new("claude")
        .arg("--version")
        .env_remove("CLAUDECODE")
        .output()
        .map_err(|e| UsageError::VersionCommand(e.to_string()))?;

    if !output.status.success() {
        return Err(UsageError::VersionCommand(format!(
            "exit code {}",
            output.status
        )));
    }

    let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Output is like "2.1.71 (Claude Code)" — extract just the version.
    Ok(version_str
        .split_whitespace()
        .next()
        .unwrap_or(&version_str)
        .to_string())
}

/// OAuth credentials structure stored in Keychain or credentials file.
#[derive(Deserialize)]
pub(crate) struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    pub(crate) claude_ai_oauth: OAuthToken,
}

#[derive(Deserialize)]
pub(crate) struct OAuthToken {
    #[serde(rename = "accessToken")]
    pub(crate) access_token: String,
}

/// Try to get token from macOS Keychain.
#[cfg(target_os = "macos")]
fn get_token_from_keychain() -> Result<String, UsageError> {
    let output = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .map_err(|e| UsageError::CredentialRead(format!("keychain command failed: {e}")))?;

    if !output.status.success() {
        return Err(UsageError::CredentialRead(
            "no credentials found in keychain".to_string(),
        ));
    }

    let json_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let creds: Credentials = serde_json::from_str(&json_str)
        .map_err(|e| UsageError::CredentialParse(format!("keychain JSON: {e}")))?;
    Ok(creds.claude_ai_oauth.access_token)
}

#[cfg(not(target_os = "macos"))]
fn get_token_from_keychain() -> Result<String, UsageError> {
    Err(UsageError::CredentialRead(
        "keychain not available on this platform".to_string(),
    ))
}

/// Try to get token from ~/.claude/.credentials.json file.
fn get_token_from_file() -> Result<String, UsageError> {
    let home = dirs::home_dir()
        .ok_or_else(|| UsageError::CredentialRead("could not determine home directory".into()))?;
    let creds_path = home.join(".claude/.credentials.json");
    let contents = std::fs::read_to_string(&creds_path)
        .map_err(|e| UsageError::CredentialRead(format!("{}: {e}", creds_path.display())))?;
    let creds: Credentials = serde_json::from_str(&contents)
        .map_err(|e| UsageError::CredentialParse(format!("credentials file: {e}")))?;
    Ok(creds.claude_ai_oauth.access_token)
}

/// Retrieve the OAuth access token, trying Keychain first then credentials file.
fn get_access_token() -> Result<String, UsageError> {
    get_token_from_keychain().or_else(|_| get_token_from_file())
}

/// HTTP timeout for the usage API request.
const USAGE_API_TIMEOUT: Duration = Duration::from_secs(10);

/// Fetch usage data from the Anthropic OAuth usage endpoint.
pub fn fetch_usage_data() -> Result<UsageData, UsageError> {
    let token = get_access_token()?;

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(USAGE_API_TIMEOUT))
            .build(),
    );

    let response = agent
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .call()
        .map_err(|e| UsageError::HttpRequest(e.to_string()))?;

    response
        .into_body()
        .read_json::<UsageData>()
        .map_err(|e| UsageError::ResponseParse(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_bar_info_defaults_to_none() {
        let info = TitleBarInfo::new();
        assert!(info.claude_version.is_none());
        assert!(info.usage.is_none());
    }

    #[test]
    fn title_bar_info_default_trait_matches_new() {
        let a = TitleBarInfo::new();
        let b = TitleBarInfo::default();
        assert!(a.claude_version.is_none());
        assert!(b.claude_version.is_none());
        assert!(a.usage.is_none());
        assert!(b.usage.is_none());
    }

    #[test]
    fn usage_window_deserializes() {
        let json = r#"{"utilization": 42.5, "resets_at": "2026-03-08T12:00:00Z"}"#;
        let window: UsageWindow = serde_json::from_str(json).unwrap();
        assert!((window.utilization - 42.5).abs() < f64::EPSILON);
        assert!(window.resets_at.is_some());
        assert_eq!(
            window
                .resets_at
                .unwrap()
                .to_rfc3339()
                .contains("2026-03-08"),
            true
        );
    }

    #[test]
    fn usage_window_deserializes_with_timezone_offset() {
        let json = r#"{"utilization": 10.0, "resets_at": "2026-03-09T00:00:00.868849+00:00"}"#;
        let window: UsageWindow = serde_json::from_str(json).unwrap();
        assert!(window.resets_at.is_some());
    }

    #[test]
    fn usage_window_deserializes_null_reset() {
        let json = r#"{"utilization": 0.0, "resets_at": null}"#;
        let window: UsageWindow = serde_json::from_str(json).unwrap();
        assert!((window.utilization - 0.0).abs() < f64::EPSILON);
        assert!(window.resets_at.is_none());
    }

    #[test]
    fn usage_data_deserializes_full_response() {
        let json = r#"{
            "five_hour": {"utilization": 6.0, "resets_at": "2026-03-08T05:00:00Z"},
            "seven_day": {"utilization": 35.0, "resets_at": "2026-03-10T04:00:00Z"},
            "seven_day_oauth_apps": null,
            "seven_day_opus": {"utilization": 0.0, "resets_at": null},
            "iguana_necktie": null
        }"#;
        let data: UsageData = serde_json::from_str(json).unwrap();
        assert!((data.five_hour.as_ref().unwrap().utilization - 6.0).abs() < f64::EPSILON);
        assert!((data.seven_day.as_ref().unwrap().utilization - 35.0).abs() < f64::EPSILON);
        assert!((data.seven_day_opus.as_ref().unwrap().utilization - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn usage_data_deserializes_minimal_response() {
        let json = r#"{"five_hour": null, "seven_day": null, "seven_day_opus": null}"#;
        let data: UsageData = serde_json::from_str(json).unwrap();
        assert!(data.five_hour.is_none());
        assert!(data.seven_day.is_none());
    }

    #[test]
    fn usage_data_deserializes_with_missing_fields() {
        // API might omit fields entirely (not send null).
        // #[serde(default)] ensures this works.
        let json = r#"{"five_hour": {"utilization": 10.0, "resets_at": null}}"#;
        let data: UsageData = serde_json::from_str(json).unwrap();
        assert!(data.five_hour.is_some());
        assert!(data.seven_day.is_none());
        assert!(data.seven_day_opus.is_none());
    }

    #[test]
    fn usage_data_deserializes_empty_object() {
        let json = r#"{}"#;
        let data: UsageData = serde_json::from_str(json).unwrap();
        assert!(data.five_hour.is_none());
        assert!(data.seven_day.is_none());
        assert!(data.seven_day_opus.is_none());
    }

    #[test]
    fn credentials_deserializes_with_renamed_fields() {
        let json = r#"{
            "claudeAiOauth": {
                "accessToken": "sk-ant-oat01-test-token",
                "refreshToken": "sk-ant-ort01-refresh",
                "expiresAt": 1773039614560,
                "scopes": ["user:inference"],
                "subscriptionType": "max"
            }
        }"#;
        let creds: Credentials = serde_json::from_str(json).unwrap();
        assert_eq!(
            creds.claude_ai_oauth.access_token,
            "sk-ant-oat01-test-token"
        );
    }

    #[test]
    fn version_parsing_extracts_number() {
        // Simulates parsing "2.1.71 (Claude Code)"
        let version_str = "2.1.71 (Claude Code)";
        let version = version_str.split_whitespace().next().unwrap();
        assert_eq!(version, "2.1.71");
    }

    #[test]
    fn usage_error_displays_version_command() {
        let err = UsageError::VersionCommand("not found".to_string());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn usage_error_displays_credential_read() {
        let err = UsageError::CredentialRead("permission denied".to_string());
        assert!(err.to_string().contains("permission denied"));
    }

    #[test]
    fn usage_error_displays_http_request() {
        let err = UsageError::HttpRequest("timeout".to_string());
        assert!(err.to_string().contains("timeout"));
    }
}
