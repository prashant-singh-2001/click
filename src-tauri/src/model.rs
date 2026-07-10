use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchStrategy {
    #[default]
    Sequential,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    App {
        id: Uuid,
        label: String,
        path: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default)]
        delay_after_ms: Option<u64>,
    },
    Url {
        id: Uuid,
        label: String,
        url: String,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default)]
        delay_after_ms: Option<u64>,
    },
}

impl Action {
    pub fn id(&self) -> Uuid {
        match self {
            Action::App { id, .. } => *id,
            Action::Url { id, .. } => *id,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Action::App { label, .. } => label,
            Action::Url { label, .. } => label,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            Action::App { enabled, .. } => *enabled,
            Action::Url { enabled, .. } => *enabled,
        }
    }

    pub fn delay_after_ms(&self) -> Option<u64> {
        match self {
            Action::App { delay_after_ms, .. } => *delay_after_ms,
            Action::Url { delay_after_ms, .. } => *delay_after_ms,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    #[serde(default)]
    pub launch_strategy: LaunchStrategy,
    #[serde(default = "default_delay_ms")]
    pub default_delay_ms: u64,
    #[serde(default)]
    pub hotkey: Option<String>,
    #[serde(default)]
    pub actions: Vec<Action>,
}

fn default_delay_ms() -> u64 {
    300
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub version: u32,
    pub workspaces: Vec<Workspace>,
}

impl Default for WorkspaceFile {
    fn default() -> Self {
        WorkspaceFile {
            version: 1,
            workspaces: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for a real bug: `rename_all` on an enum renames
    /// variant names ("App" -> "app") but NOT the fields inside each
    /// variant, so `delay_after_ms` was silently staying snake_case while
    /// every other field in the schema (including the frontend's TS types)
    /// used camelCase. The frontend always sends/expects `delayAfterMs`,
    /// so the mismatch silently dropped that value to null on every save.
    #[test]
    fn action_fields_serialize_as_camel_case() {
        let action = Action::App {
            id: Uuid::new_v4(),
            label: "Test".to_string(),
            path: "C:/test.exe".to_string(),
            args: vec![],
            cwd: None,
            enabled: true,
            delay_after_ms: Some(500),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["delayAfterMs"], 500);
        assert!(json.get("delay_after_ms").is_none());
    }
}
