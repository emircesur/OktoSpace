//! What OktoSpace did, and when: installs, updates, builds, rollbacks, repairs, uninstalls and
//! failures, one JSON object per line in `history.jsonl` in the OktoSpace root.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Kept when the file grows past this many entries (the oldest are dropped).
const KEEP: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Installed,
    Updated,
    Built,
    RolledBack,
    Repaired,
    Uninstalled,
    Skipped,
    Failed,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Action::Installed => "Installed",
            Action::Updated => "Updated",
            Action::Built => "Built from source",
            Action::RolledBack => "Rolled back",
            Action::Repaired => "Repaired",
            Action::Uninstalled => "Uninstalled",
            Action::Skipped => "Skipped",
            Action::Failed => "Failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Unix seconds.
    pub at: u64,
    pub app: String,
    pub action: Action,
    /// The version it went to (or the one removed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The version it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// For failures, what went wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Event {
    pub fn new(app: &str, action: Action) -> Event {
        Event { at: crate::github::now_secs(), app: app.into(), action, version: None, from: None, detail: None }
    }

    pub fn version(mut self, v: impl ToString) -> Event {
        self.version = Some(v.to_string());
        self
    }

    pub fn from(mut self, v: Option<impl ToString>) -> Event {
        self.from = v.map(|v| v.to_string());
        self
    }

    pub fn detail(mut self, d: impl ToString) -> Event {
        self.detail = Some(d.to_string());
        self
    }

    /// "Updated 1.2.0 › 1.3.0", "Installed 0.5.0", "Failed: no space left".
    pub fn summary(&self) -> String {
        let mut s = self.action.label().to_string();
        match (&self.from, &self.version) {
            (Some(from), Some(to)) if from != to => s.push_str(&format!(" {from} › {to}")),
            (_, Some(v)) => s.push_str(&format!(" {v}")),
            _ => {}
        }
        if let Some(d) = &self.detail {
            s.push_str(&format!(": {d}"));
        }
        s
    }
}

/// Add an event to the history file.
pub fn append(path: &Path, event: &Event) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_string(event)?;
    line.push('\n');
    std::fs::OpenOptions::new().create(true).append(true).open(path)?.write_all(line.as_bytes())?;
    // Trim now and then, so the file doesn't grow forever.
    if std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) > (KEEP as u64) * 400 {
        let mut all = load(path);
        if all.len() > KEEP {
            all.drain(..all.len() - KEEP);
            let text: String = all.iter().filter_map(|e| serde_json::to_string(e).ok()).map(|l| l + "\n").collect();
            crate::paths::write_atomic(path, text.as_bytes())?;
        }
    }
    Ok(())
}

/// Every event, oldest first (unreadable lines are skipped).
pub fn load(path: &Path) -> Vec<Event> {
    std::fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// "just now", "5 minutes ago", "3 days ago", "2 years ago".
pub fn ago(secs: u64, now: u64) -> String {
    let d = now.saturating_sub(secs);
    let (n, unit) = match d {
        0..=59 => return "just now".into(),
        60..=3599 => (d / 60, "minute"),
        3600..=86_399 => (d / 3600, "hour"),
        86_400..=2_591_999 => (d / 86_400, "day"),
        2_592_000..=31_535_999 => (d / 2_592_000, "month"),
        _ => (d / 31_536_000, "year"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        append(&path, &Event::new("zed", Action::Installed).version("1.0.0")).unwrap();
        append(&path, &Event::new("zed", Action::Updated).version("1.1.0").from(Some("1.0.0"))).unwrap();
        std::fs::write(&path, std::fs::read_to_string(&path).unwrap() + "not json\n").unwrap();
        let all = load(&path);
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].summary(), "Updated 1.0.0 › 1.1.0");
        assert_eq!(Event::new("x", Action::Failed).detail("offline").summary(), "Failed: offline");
    }

    #[test]
    fn says_how_long_ago() {
        assert_eq!(ago(100, 130), "just now");
        assert_eq!(ago(0, 3 * 86_400 + 5), "3 days ago");
        assert_eq!(ago(0, 3600), "1 hour ago");
        assert_eq!(ago(0, 2 * 31_536_000), "2 years ago");
    }
}
