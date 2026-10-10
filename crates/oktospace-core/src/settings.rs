//! User preferences, stored as `settings.json` in the OktoSpace root.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::write_atomic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Offer release candidates and other pre-releases.
    pub include_prereleases: bool,
    /// Look for updates on start and periodically while running.
    pub auto_check_updates: bool,
    /// Install updates as soon as they are found.
    pub auto_install_updates: bool,
    pub check_interval_hours: u32,
    /// Keep the previously installed version on disk so an update can be rolled back.
    pub keep_previous_version: bool,
    /// On Windows, prefer the MSI installer over the portable build.
    pub prefer_system_installer: bool,
    /// The user has picked portable or installer (asked before the first install).
    pub install_mode_chosen: bool,
    /// Find library apps installed without OktoSpace and keep them up to date where they are.
    pub detect_installed: bool,
    /// Update OktoSpace itself in the background (the new version runs from the next start).
    pub auto_update_self: bool,
    /// Where apps get installed; defaults to the `apps` folder in the OktoSpace root.
    pub install_dir: Option<PathBuf>,
    /// Add a desktop shortcut next to the Start menu / app menu entry.
    pub desktop_shortcuts: bool,
    /// Optional GitHub token, which raises the API rate limit from 60 to 5000 requests an hour.
    pub github_token: Option<String>,
    pub theme: Theme,
    /// Fetch the app list from the OktoSpace repository so new apps appear without an update.
    pub remote_catalog: bool,
    /// Offer to install OktoSpace itself when it runs from somewhere else (e.g. Downloads).
    pub offer_self_install: bool,
    /// Install packages that have no published SHA-256 (not recommended).
    pub allow_unverified_downloads: bool,
    /// Cap on total download speed, in kilobytes per second.
    pub download_limit_kbps: Option<u32>,
    /// How many apps download and install at the same time.
    pub max_parallel_downloads: u32,
    /// Linux: install AppImages instead of tarballs, so updates can download only what changed.
    pub prefer_appimage: bool,
    /// Per-app update channel; apps not listed follow `include_prereleases`.
    pub channels: BTreeMap<String, Channel>,
    /// Start OktoSpace (in the background) when you log in.
    pub start_at_login: bool,
    /// Closing the window keeps OktoSpace running in the system tray / menu bar.
    pub keep_running_in_tray: bool,
    /// Show system notifications for updates.
    pub notifications: bool,
    /// Apps added from GitHub, and other update sources.
    pub other_sources: OtherSources,
    /// A folder shared by several computers (a classroom's file share): packages are taken from
    /// it when there, instead of downloading them again.
    pub package_cache: Option<PathBuf>,
    /// Also put downloaded packages into `package_cache` for the other computers.
    pub package_cache_write: bool,
    /// macOS: how the Dock shows OktoSpace's icon.
    pub dock_icon: DockIcon,
    /// Apps marked as favorites (their new releases are checked and announced, installed or not).
    pub favorites: Vec<String>,
    /// Per-app build options (what to build and how), by app id.
    pub builds: BTreeMap<String, BuildOptions>,
    /// Updates the user skipped: no update is offered until a release newer than this one.
    pub skipped: BTreeMap<String, semver::Version>,
    /// The download to pick, by app id: part of the file name (e.g. `x86_64.AppImage`), used
    /// instead of OktoSpace's own choice when a release has a file with it in its name.
    pub assets: BTreeMap<String, String>,
    /// How to start each app: arguments and environment variables.
    pub launch: BTreeMap<String, LaunchOptions>,
}

/// Arguments and environment variables an app is started with.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchOptions {
    /// Arguments, as typed in a shell (quotes group words).
    pub args: String,
    /// `NAME=value` lines.
    pub env: String,
}

impl LaunchOptions {
    pub fn is_empty(&self) -> bool {
        self.args.trim().is_empty() && self.env.trim().is_empty()
    }

    /// The arguments split into words: `--a "b c"` → `["--a", "b c"]`.
    pub fn arg_list(&self) -> Vec<String> {
        let (mut out, mut word, mut quote, mut any) = (Vec::new(), String::new(), None::<char>, false);
        for c in self.args.chars() {
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), c) => word.push(c),
                (None, '"' | '\'') => (quote, any) = (Some(c), true),
                (None, c) if c.is_whitespace() => {
                    if any || !word.is_empty() {
                        out.push(std::mem::take(&mut word));
                    }
                    any = false;
                }
                (None, c) => word.push(c),
            }
        }
        if any || !word.is_empty() {
            out.push(word);
        }
        out
    }

    /// The `NAME=value` lines as pairs (blank lines and `#` comments are skipped).
    pub fn env_list(&self) -> Vec<(String, String)> {
        self.env
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
            .filter(|(k, _)| !k.is_empty())
            .collect()
    }
}

/// How to build an app from source, when the defaults aren't wanted.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildOptions {
    /// A branch, tag or commit to build instead of the latest release.
    pub git_ref: Option<String>,
    /// Commands to run instead of the detected build (one per line, run in the source folder).
    pub commands: Option<String>,
    /// Where the program ends up, relative to the source (when the commands put it elsewhere).
    pub output: Option<String>,
}

/// The Dock icon on macOS. When macOS draws it (the icon in the app), it follows the icon style
/// chosen in System Settings › Appearance (dark, clear or tinted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockIcon {
    /// Full colour while the window is open; macOS's style when closed or in the background.
    #[default]
    ColorWhenOpen,
    /// Always full colour while OktoSpace runs.
    Color,
    /// Always macOS's style.
    System,
}

/// Optional: apps from any GitHub repository, and other repositories (forks, mirrors, backups)
/// for library apps and for OktoSpace itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OtherSources {
    pub enabled: bool,
    /// Apps added from GitHub repositories.
    pub apps: Vec<CustomApp>,
    /// Library app id → the repository to update it from instead of the official one.
    pub overrides: BTreeMap<String, String>,
    /// Where OktoSpace looks for its own updates instead of the official repository.
    pub self_repo: Option<String>,
}

impl Default for OtherSources {
    fn default() -> Self {
        // On: adding any repository's app is part of what OktoSpace does.
        OtherSources { enabled: true, apps: Vec::new(), overrides: BTreeMap::new(), self_repo: None }
    }
}

/// An app added from a GitHub repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomApp {
    pub id: String,
    pub name: String,
    /// `owner/repo`.
    pub repo: String,
    /// The program's file name, when it isn't the repository's name (`rg` for ripgrep).
    #[serde(default)]
    pub binary: Option<String>,
}

/// Which releases an app follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", tag = "channel", content = "version")]
pub enum Channel {
    /// Follow the global "offer pre-releases" setting.
    #[default]
    Default,
    Stable,
    Prerelease,
    /// Stay on this version: no updates are offered.
    Pinned(semver::Version),
    /// Build the newest commit on the default branch, and rebuild when there's a newer one.
    Commits,
}

impl Channel {
    pub fn label(&self) -> String {
        match self {
            Channel::Default => "Default".into(),
            Channel::Stable => "Stable".into(),
            Channel::Prerelease => "Pre-release".into(),
            Channel::Pinned(v) => format!("Stay on {v}"),
            Channel::Commits => "Latest commit".into(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            include_prereleases: false,
            auto_check_updates: true,
            auto_install_updates: false,
            check_interval_hours: 6,
            keep_previous_version: true,
            prefer_system_installer: false,
            install_mode_chosen: false,
            detect_installed: true,
            auto_update_self: true,
            install_dir: None,
            desktop_shortcuts: false,
            github_token: None,
            theme: Theme::Dark,
            remote_catalog: true,
            offer_self_install: true,
            allow_unverified_downloads: false,
            download_limit_kbps: None,
            max_parallel_downloads: 2,
            prefer_appimage: false,
            channels: BTreeMap::new(),
            start_at_login: false,
            keep_running_in_tray: true,
            notifications: true,
            other_sources: OtherSources::default(),
            package_cache: None,
            package_cache_write: false,
            dock_icon: DockIcon::default(),
            favorites: Vec::new(),
            builds: BTreeMap::new(),
            skipped: BTreeMap::new(),
            assets: BTreeMap::new(),
            launch: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Load settings, falling back to defaults when the file is missing or unreadable.
    pub fn load(path: &Path) -> Settings {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                log::warn!("ignoring unreadable {}: {err}", path.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        write_atomic(path, &serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn channel(&self, app_id: &str) -> Channel {
        self.channels.get(app_id).cloned().unwrap_or_default()
    }

    /// Whether `app_id` should be offered pre-releases.
    pub fn wants_prereleases(&self, app_id: &str) -> bool {
        match self.channel(app_id) {
            Channel::Default => self.include_prereleases,
            Channel::Prerelease => true,
            Channel::Stable | Channel::Pinned(_) | Channel::Commits => false,
        }
    }

    pub fn asset_prefs(&self, app: &crate::catalog::AppEntry) -> crate::platform::AssetPrefs {
        crate::platform::AssetPrefs {
            prefer_system_installer: self.prefer_system_installer,
            prefer_exe_installer: app.windows_installer.as_deref() == Some("exe"),
            prefer_appimage: self.prefer_appimage,
            linux_package: crate::platform::linux_package_format(),
        }
    }

    /// The GitHub token from settings, or `GITHUB_TOKEN` / `GH_TOKEN` in the environment.
    pub fn effective_github_token(&self) -> Option<String> {
        self.github_token
            .clone()
            .filter(|t| !t.trim().is_empty())
            .or_else(|| std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty()))
            .or_else(|| std::env::var("GH_TOKEN").ok().filter(|t| !t.is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());

        let s = Settings { include_prereleases: true, ..Settings::default() };
        s.save(&path).unwrap();
        assert!(Settings::load(&path).include_prereleases);

        let pinned = Settings {
            channels: [("photocraft".to_string(), Channel::Pinned(semver::Version::new(0, 3, 0)))].into(),
            ..Settings::default()
        };
        pinned.save(&path).unwrap();
        assert_eq!(Settings::load(&path).channel("photocraft"), Channel::Pinned(semver::Version::new(0, 3, 0)));
        assert!(!pinned.wants_prereleases("photocraft"));
        assert_eq!(pinned.channel("gridcraft"), Channel::Default);

        std::fs::write(&path, r#"{"auto_install_updates": true}"#).unwrap();
        let s = Settings::load(&path);
        assert!(s.auto_install_updates);
        assert!(s.auto_check_updates);
    }

    #[test]
    fn launch_options_split_like_a_shell() {
        let o = LaunchOptions {
            args: r#"--profile "my work" -v '' x"#.into(),
            env: "# c\nA=1\n\nB = two words\nbad".into(),
        };
        assert_eq!(o.arg_list(), ["--profile", "my work", "-v", "", "x"]);
        assert_eq!(o.env_list(), [("A".to_string(), "1".to_string()), ("B".to_string(), "two words".to_string())]);
        assert!(LaunchOptions::default().is_empty());
        // The new channel is stored like the others.
        let s = Settings { channels: [("x".to_string(), Channel::Commits)].into(), ..Settings::default() };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.channel("x"), Channel::Commits);
    }
}
