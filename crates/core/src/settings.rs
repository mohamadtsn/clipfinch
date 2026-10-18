use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryLimit {
    Day,
    Week,
    Month,
    Year,
    Forever,
}

impl HistoryLimit {
    pub const ALL: [HistoryLimit; 5] = [
        HistoryLimit::Day,
        HistoryLimit::Week,
        HistoryLimit::Month,
        HistoryLimit::Year,
        HistoryLimit::Forever,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HistoryLimit::Day => "One day",
            HistoryLimit::Week => "One week",
            HistoryLimit::Month => "One month",
            HistoryLimit::Year => "One year",
            HistoryLimit::Forever => "Forever",
        }
    }

    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(2) as u32
    }

    pub fn from_index(i: u32) -> HistoryLimit {
        Self::ALL
            .get(i as usize)
            .copied()
            .unwrap_or(HistoryLimit::Month)
    }

    /// Maximum age of an unpinned clip in milliseconds (`None` = keep forever).
    pub fn millis(self) -> Option<i64> {
        const DAY: i64 = 86_400_000;
        match self {
            HistoryLimit::Day => Some(DAY),
            HistoryLimit::Week => Some(7 * DAY),
            HistoryLimit::Month => Some(30 * DAY),
            HistoryLimit::Year => Some(365 * DAY),
            HistoryLimit::Forever => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub history_limit: HistoryLimit,
    /// Window classes (e.g. `keepassxc`) whose copies are never recorded.
    pub excluded_apps: Vec<String>,
    pub paused: bool,
    pub open_at_login: bool,
    /// Send Ctrl+Shift+V instead of Ctrl+V (terminals).
    pub paste_with_shift: bool,
    pub popup_hotkey: String,
    pub queue_hotkey: String,
    pub queue_paste_hotkey: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            history_limit: HistoryLimit::Month,
            excluded_apps: Vec::new(),
            paused: false,
            open_at_login: false,
            paste_with_shift: false,
            popup_hotkey: "<Super><Shift>v".to_string(),
            queue_hotkey: "<Super><Shift>c".to_string(),
            queue_paste_hotkey: "<Super><Shift>x".to_string(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults() {
        let dir = std::env::temp_dir().join(format!("clipfinch-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut s = Settings::default();
        s.history_limit = HistoryLimit::Week;
        s.excluded_apps = vec!["keepassxc".into()];
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        // Missing fields fall back to defaults.
        fs::write(&path, r#"{"paused": true}"#).unwrap();
        let l = Settings::load(&path);
        assert!(l.paused);
        assert_eq!(l.history_limit, HistoryLimit::Month);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn limits() {
        assert_eq!(HistoryLimit::from_index(HistoryLimit::Year.index()), HistoryLimit::Year);
        assert!(HistoryLimit::Forever.millis().is_none());
    }
}
