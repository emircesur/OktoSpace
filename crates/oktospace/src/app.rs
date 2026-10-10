//! The window: top bar, sidebars, routing between views, the job queue, the tray and
//! notifications.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke, Vec2};
use egui_commonmark::CommonMarkCache;
use oktospace_core::manager::{AppList, VerifyReport};
use oktospace_core::selfupdate::{self, SelfUpdate};
use oktospace_core::settings::Channel;
use oktospace_core::{AppState, Manager, ProgressEvent, Settings};
use semver::Version;

use crate::theme::{self, Palette};
use crate::tray::{Tray, TrayCommand};
use crate::views;
use crate::worker::{self, Bus, Job, JobKind, Msg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Apps,
    Discover,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppsView {
    All,
    Updates,
    Installed,
    Category(String),
    Detail(String),
    Favorites,
    /// What OktoSpace did: installs, updates, builds, failures.
    Activity,
}

/// How the App library is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibrarySort {
    Popular,
    Name,
    /// Newest release first (apps whose releases are known).
    Recent,
}

/// Which apps the App library shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryFilter {
    All,
    /// Something to download for this computer.
    ForThisComputer,
    BuildsFromSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Overview,
    Versions,
    Readme,
    Build,
}

/// Things views ask for; handled after drawing so views only need shared access.
#[derive(Debug, Clone)]
pub enum Action {
    Install(String, Option<Version>),
    /// Install into a folder the user picks.
    InstallTo(String),
    InstallMany(Vec<String>),
    Update(String),
    UpdateAll,
    AskUninstall(String),
    /// Settings › IT & Classroom.
    SaveReport,
    FillCache,
    Uninstall(String),
    Rollback(String),
    /// Build from source and install (the Build tab shows the log).
    Build(String),
    ToggleFavorite(String),
    SkipVersion(String),
    Unskip(String),
    /// Pick the download by part of its file name (`None`: automatic).
    SetAssetChoice(String, Option<String>),
    SaveLaunchOptions(String, oktospace_core::settings::LaunchOptions),
    /// Delete the earlier version kept for rollback.
    TrimPrevious(String),
    /// The command palette (Ctrl+K).
    OpenPalette,
    /// Save an app's build options (Build tab).
    SaveBuildOptions(String, oktospace_core::settings::BuildOptions),
    /// The Find on GitHub window: open it, search, load a user's stars, add a repository.
    OpenGithub,
    SearchGithub(String),
    LoadStars(String),
    AddRepo(String),
    VerifyAndRepair(String),
    Cancel(String),
    Launch(String, Vec<PathBuf>),
    OpenUrl(String),
    OpenPath(PathBuf),
    ShowApp(String),
    GoApps(AppsView),
    Refresh,
    OpenSettings,
    SaveSettings(Box<Settings>),
    ApplySelfUpdate,
    SelfInstall,
    DismissSelfInstall,
    Cleanup,
    TestNotification,
    OpenAbout,
    /// Open the other-sources dialog.
    OpenSource(views::sources::Target),
    Source(worker::SourceOp),
    /// Start the updated OktoSpace and quit this one.
    RestartOktoSpace,
    /// The answer to "portable or installer?" (true: installer).
    SetInstallMode(bool),
    SetChannel(String, Channel),
    ReportBug(String),
    ExportList,
    ImportList,
    OpenMultiInstall,
    /// From the "app is open" dialog.
    WhenClosed(String, JobKind),
    RunNow(String, JobKind),
}

#[derive(Clone, Copy, PartialEq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub at: Instant,
}

pub struct OktoSpaceApp {
    pub manager: Manager,
    pub bus: Bus,
    rx: Receiver<Msg>,
    pub palette: Palette,
    applied_theme: Option<(oktospace_core::settings::Theme, bool)>,

    pub tab: Tab,
    pub apps_view: AppsView,
    /// Where "Back" goes from a detail page.
    pub back_view: AppsView,
    pub detail_tab: DetailTab,
    pub search: String,

    pub states: Vec<AppState>,
    /// Running and queued jobs, by app id (or `addon:<id>`), in queue order.
    pub jobs: HashMap<String, Job>,
    pub job_order: Vec<String>,
    /// Jobs waiting for an app to be closed.
    pub waiting_for_close: Vec<(String, JobKind)>,
    last_close_check: Instant,
    /// The "app is open" question.
    pub running_prompt: Option<(String, JobKind)>,
    pub multi_install: Option<BTreeSet<String>>,
    pub downloads_open: bool,

    pub refreshing: bool,
    pub refresh_errors: Vec<(String, String)>,
    pub last_check: Option<Instant>,
    pub self_update: Option<SelfUpdate>,
    pub self_updating: bool,
    /// A newer OktoSpace is in place; it runs from the next start.
    pub self_update_ready: Option<semver::Version>,
    required_queued: bool,
    /// Pinned versions queued this run (app ids).
    pinned_queued: HashSet<String>,

    pub readmes: HashMap<String, Result<String, String>>,
    readme_loading: HashSet<String>,
    /// Apps whose releases are being fetched, or couldn't be (with why).
    pub releases_loading: HashSet<String>,
    pub releases_failed: HashMap<String, String>,
    /// Each app's latest build log.
    pub build_logs: HashMap<String, Vec<String>>,
    pub library_sort: LibrarySort,
    /// The latest release seen of each favorite (to announce new ones).
    favorite_seen: HashMap<String, String>,
    pub library_filter: LibraryFilter,
    /// The Find on GitHub window.
    pub github: Option<views::github::Dialog>,
    /// The build options being edited on the Build tab (app id, options).
    pub build_draft: Option<(String, oktospace_core::settings::BuildOptions)>,
    /// The launch options being edited on an app page.
    pub launch_draft: Option<(String, oktospace_core::settings::LaunchOptions)>,
    /// The command palette (Ctrl+K), when open.
    pub palette_open: Option<views::palette::CommandPalette>,
    /// Settings › Storage and Build tools, measured when Settings opens.
    pub storage: Option<oktospace_core::manager::Storage>,
    pub toolchains: Option<Vec<(oktospace_core::build::Tool, Option<PathBuf>)>>,
    pub verify_results: HashMap<String, VerifyReport>,
    pub tours: HashMap<String, Result<oktospace_core::tour::Tour, String>>,
    tour_loading: HashSet<String>,
    pub tour_index: HashMap<String, usize>,
    pub shots: HashMap<String, Result<egui::TextureHandle, String>>,
    shots_loading: HashSet<String>,

    pub settings: Settings,
    pub settings_draft: Option<Settings>,
    pub confirm_uninstall: Option<String>,
    confirm_quit: bool,
    pub md_cache: CommonMarkCache,
    pub detail_release: Option<String>,
    /// Running from outside the OktoSpace folder (e.g. straight from Downloads).
    pub offer_self_install: bool,
    toasts: Vec<Toast>,
    pub actions: Vec<Action>,

    tray: Option<Tray>,
    tray_rx: Receiver<TrayCommand>,
    hidden: bool,
    pub about_open: bool,
    /// Settings › Other sources: the add / change repository dialog.
    pub source_dialog: Option<views::sources::Dialog>,
    dock: crate::dock::Dock,
    /// An install waiting for the one-time "portable or installer?" answer.
    pub install_mode_pending: Option<Action>,
    /// Hidden while no tray icon is showing (the panel isn't there yet, or went away).
    hidden_without_tray: Option<Instant>,
    quitting: bool,
    told_about_tray: bool,
    /// Whether the window had the focus last frame.
    was_focused: bool,
}

impl OktoSpaceApp {
    pub fn new(cc: &eframe::CreationContext<'_>, manager: Manager, background: bool) -> OktoSpaceApp {
        let (tx, rx) = channel();
        let bus = Bus::new(tx, cc.egui_ctx.clone());
        let settings = manager.settings();
        let palette = theme::palette_for(settings.theme, &cc.egui_ctx);
        let (tray_tx, tray_rx) = channel();
        let tray = Tray::new(&cc.egui_ctx, tray_tx);
        if tray.is_some() {
            log::info!("tray icon ready");
        }
        worker::heartbeat(cc.egui_ctx.clone());
        let mut app = OktoSpaceApp {
            states: manager.states(),
            bus,
            rx,
            palette,
            applied_theme: None,
            tab: Tab::Apps,
            apps_view: AppsView::All,
            back_view: AppsView::All,
            detail_tab: DetailTab::Overview,
            search: String::new(),
            jobs: HashMap::new(),
            job_order: Vec::new(),
            waiting_for_close: Vec::new(),
            last_close_check: Instant::now(),
            running_prompt: None,
            multi_install: None,
            downloads_open: false,
            refreshing: false,
            refresh_errors: Vec::new(),
            last_check: None,
            self_update: None,
            self_updating: false,
            self_update_ready: None,
            required_queued: false,
            pinned_queued: Default::default(),
            readmes: HashMap::new(),
            readme_loading: HashSet::new(),
            releases_loading: HashSet::new(),
            releases_failed: HashMap::new(),
            build_logs: HashMap::new(),
            library_sort: LibrarySort::Popular,
            favorite_seen: HashMap::new(),
            library_filter: LibraryFilter::All,
            github: None,
            build_draft: None,
            launch_draft: None,
            palette_open: None,
            storage: None,
            toolchains: None,
            verify_results: HashMap::new(),
            tours: HashMap::new(),
            tour_loading: HashSet::new(),
            tour_index: HashMap::new(),
            shots: HashMap::new(),
            shots_loading: HashSet::new(),
            settings,
            settings_draft: None,
            confirm_uninstall: None,
            confirm_quit: false,
            md_cache: CommonMarkCache::default(),
            detail_release: None,
            offer_self_install: false,
            toasts: Vec::new(),
            actions: Vec::new(),
            tray,
            tray_rx,
            hidden: false,
            about_open: false,
            source_dialog: None,
            dock: Default::default(),
            install_mode_pending: None,
            hidden_without_tray: None,
            quitting: false,
            told_about_tray: false,
            was_focused: false,
            manager,
        };
        theme::load_icons(&cc.egui_ctx, &app.manager, None);
        app.offer_self_install = app.settings.offer_self_install && !selfupdate::is_installed_copy(&app.manager);
        if background {
            if app.tray.is_some() && can_hide_windows() {
                app.hidden = true;
            } else {
                app.hidden = app.tray.is_some();
                // Nowhere to hide: start minimized instead.
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        }
        // `OKTOSPACE_VIEW=discover|updates|installed|activity|favorites|settings|palette|about|app:<id>[:tab]` opens a specific
        // page (handy for screenshots and for jumping straight somewhere from a shortcut).
        if let Ok(view) = std::env::var("OKTOSPACE_VIEW") {
            match view.as_str() {
                "discover" => app.tab = Tab::Discover,
                "updates" => app.apps_view = AppsView::Updates,
                "installed" => app.apps_view = AppsView::Installed,
                "about" => app.about_open = true,
                "activity" => app.apps_view = AppsView::Activity,
                "favorites" => app.apps_view = AppsView::Favorites,
                "settings" => app.actions.push(Action::OpenSettings),
                "palette" => app.palette_open = Some(Default::default()),
                other => {
                    if let Some(rest) = other.strip_prefix("app:") {
                        let (id, tab) = rest.split_once(':').unwrap_or((rest, ""));
                        app.apps_view = AppsView::Detail(id.to_string());
                        app.detail_tab = match tab {
                            "versions" => DetailTab::Versions,
                            "readme" => DetailTab::Readme,
                            "build" => DetailTab::Build,
                            _ => DetailTab::Overview,
                        };
                    }
                }
            }
        }
        if std::env::var_os("OKTOSPACE_TEST_NOTIFICATION").is_some() {
            worker::test_notification(&app.bus);
        }
        // Always fetch on start when nothing is cached yet; otherwise follow the setting.
        if app.settings.auto_check_updates || app.states.iter().all(|s| !s.known) {
            app.start_refresh(false, false);
        }
        app
    }

    fn start_refresh(&mut self, force: bool, manual: bool) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        self.last_check = Some(Instant::now());
        worker::refresh(&self.bus, &self.manager, force, manual);
    }

    fn tray_shown(&self) -> bool {
        self.tray.as_ref().is_some_and(Tray::shown)
    }

    /// Put the window away while OktoSpace keeps running: hidden (the tray icon brings it
    /// back), or minimized to the taskbar where windows can't be hidden (Wayland).
    fn put_away(&mut self, ctx: &egui::Context) {
        self.hidden = true;
        if can_hide_windows() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    /// A window hidden in the tray must stay reachable: if no tray icon shows up for a while
    /// (started at login on a desktop without a tray), minimize it to the taskbar instead.
    fn keep_findable(&mut self, ctx: &egui::Context) {
        if !self.hidden || self.tray_shown() || !can_hide_windows() {
            self.hidden_without_tray = None;
            return;
        }
        let since = *self.hidden_without_tray.get_or_insert_with(Instant::now);
        if since.elapsed() >= Duration::from_secs(30) {
            log::info!("no tray icon is showing; minimizing the window instead");
            self.hidden = false;
            self.hidden_without_tray = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        } else {
            ctx.request_repaint_after(Duration::from_secs(5));
        }
    }

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), kind, at: Instant::now() });
    }

    /// A toast, plus a system notification when the window isn't in front.
    fn announce(&mut self, ctx: &egui::Context, kind: ToastKind, text: String) {
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if self.settings.notifications && (self.hidden || !focused) {
            worker::notify(text.clone());
        }
        self.toast(kind, text);
    }

    pub fn state(&self, id: &str) -> Option<&AppState> {
        self.states.iter().find(|s| s.app.id == id)
    }

    pub fn update_count(&self) -> usize {
        self.states.iter().filter(|s| s.update_available).count()
    }

    fn reload_states(&mut self) {
        self.states = self.manager.states();
        if let Some(tray) = &self.tray {
            tray.set_updates(self.update_count());
        }
        // A favorite that isn't installed got a new release since it was last seen.
        let mut news = Vec::new();
        for s in &self.states {
            if !self.settings.favorites.contains(&s.app.id) || s.installed.is_some() {
                continue;
            }
            let Some(tag) = s.latest.as_ref().map(|r| r.tag.clone()) else { continue };
            match self.favorite_seen.insert(s.app.id.clone(), tag.clone()) {
                Some(old) if old != tag => {
                    news.push(format!("{} {}", s.app.name, s.latest_version().map(|v| v.to_string()).unwrap_or(tag)))
                }
                _ => {}
            }
        }
        if !news.is_empty() {
            let text = format!("New release of a favorite: {}", news.join(", "));
            if self.settings.notifications {
                worker::notify(text.clone());
            }
            self.toast(ToastKind::Info, text);
        }
    }

    /// Whether the organization keeps users from uninstalling and rolling back (and this isn't an
    /// administrator).
    pub fn managed(&self) -> bool {
        self.manager.policy().prevent_uninstall && !oktospace_core::platform::is_elevated()
    }

    /// Who manages this computer, for messages.
    pub fn organization(&self) -> String {
        self.manager.policy().organization.clone().unwrap_or_else(|| "Your organization".into())
    }

    pub fn app_name(&self, key: &str) -> String {
        self.manager.app(key).map(|a| a.name).unwrap_or_else(|| key.to_string())
    }

    fn process_messages(&mut self, ctx: &egui::Context) {
        let mut reload = false;
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Progress { key, event } => {
                    if let Some(job) = self.jobs.get_mut(&key) {
                        match event {
                            ProgressEvent::Stage(s) => job.stage = s,
                            ProgressEvent::Bytes { done, total } => {
                                job.done = done;
                                job.total = total;
                            }
                        }
                    }
                }
                Msg::JobDone { key, kind, result } => {
                    self.jobs.remove(&key);
                    self.job_order.retain(|k| *k != key);
                    reload = true;
                    match result {
                        Ok(text) => self.announce(ctx, ToastKind::Success, text),
                        Err(text) if text == "Cancelled" => self.toast(ToastKind::Info, "Cancelled"),
                        Err(text) => {
                            let name = self.app_name(&key);
                            self.announce(ctx, ToastKind::Error, format!("{} {name} failed: {text}", kind.verb()));
                        }
                    }
                    self.pump_queue();
                }
                Msg::Refreshed { errors, manual } => {
                    self.refreshing = false;
                    reload = true;
                    if manual && !errors.is_empty() {
                        self.toast(
                            ToastKind::Error,
                            format!("Couldn't check {} app(s); see the Updates page", errors.len()),
                        );
                    }
                    self.refresh_errors = errors;
                    self.after_refresh(ctx, manual);
                }
                Msg::Icons(ids) => theme::load_icons(ctx, &self.manager, Some(&ids)),
                Msg::GithubResults(result) => {
                    if let Some(d) = &mut self.github {
                        d.busy = false;
                        match result {
                            Ok(hits) => d.results = hits,
                            Err(err) => d.error = Some(err),
                        }
                    }
                }
                Msg::RepoAdded(result) => match result {
                    Ok(id) => {
                        reload = true;
                        if let Some(d) = &mut self.github {
                            d.added.insert(id.clone());
                        }
                        self.toast(ToastKind::Success, format!("{} is in your App library", self.app_name(&id)));
                    }
                    Err(err) => self.toast(ToastKind::Error, format!("Couldn't add it: {err}")),
                },
                Msg::BuildLog { key, line } => {
                    let log = self.build_logs.entry(key).or_default();
                    log.push(line);
                    if log.len() > 4000 {
                        log.drain(..1000);
                    }
                }
                Msg::Releases(id, result) => {
                    self.releases_loading.remove(&id);
                    if let Err(err) = result {
                        self.releases_failed.insert(id, err);
                    }
                    reload = true;
                }
                Msg::Readme(id, result) => {
                    self.readme_loading.remove(&id);
                    self.readmes.insert(id, result);
                }
                Msg::Verified(id, result) => match result {
                    Ok(report) if report.is_ok() => {
                        self.toast(
                            ToastKind::Success,
                            format!("{}: all {} files are intact", self.app_name(&id), report.checked),
                        );
                        self.verify_results.insert(id, report);
                    }
                    Ok(report) => {
                        let n = report.missing.len() + report.changed.len();
                        self.toast(
                            ToastKind::Info,
                            format!("{}: {n} damaged or missing file(s); repairing", self.app_name(&id)),
                        );
                        self.verify_results.insert(id.clone(), report);
                        self.request(&id, JobKind::Repair);
                    }
                    Err(err) => {
                        self.toast(ToastKind::Info, format!("Couldn't check the files ({err}); reinstalling"));
                        self.request(&id, JobKind::Repair);
                    }
                },
                Msg::Tour(id, result) => {
                    self.tour_loading.remove(&id);
                    self.tours.insert(id, result);
                }
                Msg::Shot(url, result) => {
                    self.shots_loading.remove(&url);
                    let texture = result.map(|img| ctx.load_texture(url.clone(), img, egui::TextureOptions::LINEAR));
                    self.shots.insert(url, texture);
                }
                Msg::SelfUpdateFound(found) => {
                    let found = found.map(|b| *b).filter(|u| self.self_update_ready.as_ref() != Some(&u.version));
                    self.self_update = found;
                    // Updating OktoSpace itself: in the background, used from the next start.
                    if self.settings.auto_update_self && !self.self_updating {
                        if let Some(update) = self.self_update.clone() {
                            log::info!("updating OktoSpace to {} in the background", update.version);
                            self.self_updating = true;
                            worker::apply_self_update(&self.bus, &self.manager, update);
                        }
                    }
                }
                Msg::SourceChecked(result) => {
                    if let Some(d) = self.source_dialog.as_mut() {
                        d.checking = false;
                        d.check = Some(result);
                    }
                }
                Msg::SourceDone(result) => {
                    match result {
                        Ok(text) => {
                            self.source_dialog = None;
                            self.toast(ToastKind::Success, text);
                        }
                        Err(err) => {
                            if let Some(d) = self.source_dialog.as_mut() {
                                d.working = false;
                                d.check = Some(Err(err.clone()));
                            }
                            self.toast(ToastKind::Error, err);
                        }
                    }
                    self.settings = self.manager.settings();
                    if let Some(draft) = self.settings_draft.as_mut() {
                        draft.other_sources = self.settings.other_sources.clone();
                    }
                    reload = true;
                    theme::load_icons(ctx, &self.manager, None);
                    self.start_refresh(true, false);
                }
                Msg::PolicyChanged => match Manager::open_at(self.manager.paths().clone()) {
                    Ok(manager) => {
                        self.manager = manager;
                        self.settings = self.manager.settings();
                        reload = true;
                        self.toast(
                            ToastKind::Info,
                            format!("{} updated how OktoSpace is set up here", self.organization()),
                        );
                    }
                    Err(err) => log::warn!("couldn't apply the new policy: {err:#}"),
                },
                Msg::CacheFilled(result) => match result {
                    Ok(text) => self.toast(ToastKind::Success, text),
                    Err(text) => self.toast(ToastKind::Error, format!("Couldn't fill the package cache: {text}")),
                },
                Msg::Adopted(found) => {
                    reload = true;
                    let names: Vec<String> = found
                        .iter()
                        .map(|(name, v)| match v {
                            Some(v) => format!("{name} {v}"),
                            None => name.clone(),
                        })
                        .collect();
                    self.announce(
                        ctx,
                        ToastKind::Info,
                        format!(
                            "Found {} on this computer; OktoSpace keeps it up to date where it is",
                            names.join(", ")
                        ),
                    );
                }
                Msg::TestNotification(result) => match result {
                    Ok(()) => self.toast(ToastKind::Success, "Sent a test notification"),
                    Err(err) => {
                        self.toast(ToastKind::Error, format!("This desktop didn't accept the notification: {err}"))
                    }
                },
                Msg::SelfUpdateDone(result) => {
                    self.self_updating = false;
                    match result {
                        Ok(_) => {
                            let version = self.self_update.take().map(|u| u.version);
                            self.self_update_ready.clone_from(&version);
                            // Hidden in the tray with nothing running: switch over right away.
                            if self.hidden && self.jobs.is_empty() {
                                self.actions.push(Action::RestartOktoSpace);
                            } else if let Some(v) = version {
                                self.announce(
                                    ctx,
                                    ToastKind::Success,
                                    format!("OktoSpace {v} is installed and starts next time (or restart it now from the sidebar)"),
                                );
                            }
                        }
                        Err(text) => self.toast(ToastKind::Error, format!("OktoSpace update failed: {text}")),
                    }
                }
            }
        }
        if reload {
            self.reload_states();
        }
    }

    fn after_refresh(&mut self, ctx: &egui::Context, manual: bool) {
        self.reload_states();
        // Apps a machine policy requires, once per run.
        if !self.required_queued {
            self.required_queued = true;
            for id in self.manager.required_missing() {
                self.start_job(&id, JobKind::Install(None));
            }
        }
        // Apps the organization keeps on a version they aren't on yet.
        for (id, version) in self.manager.off_pinned_version() {
            if self.pinned_queued.insert(id.clone()) {
                self.start_job(&id, JobKind::Install(Some(version)));
            }
        }
        let updates: Vec<String> =
            self.states.iter().filter(|s| s.update_available).map(|s| s.app.id.clone()).collect();
        if updates.is_empty() {
            if manual {
                self.toast(ToastKind::Info, "All your apps are up to date");
            }
            return;
        }
        let window = self.manager.policy().update_window.clone().filter(|_| !self.manager.policy().may_update_now());
        if let (true, Some(window)) = (self.settings.auto_install_updates, window) {
            let n = updates.len();
            self.announce(
                ctx,
                ToastKind::Info,
                format!(
                    "{n} update{} available; {} installs updates {}",
                    if n == 1 { "" } else { "s" },
                    self.organization(),
                    window.describe()
                ),
            );
        } else if self.settings.auto_install_updates {
            for id in updates {
                self.request_quietly(&id, JobKind::Update);
            }
        } else {
            let n = updates.len();
            self.announce(ctx, ToastKind::Info, format!("{n} update{} available", if n == 1 { "" } else { "s" }));
        }
    }

    // ---- jobs ------------------------------------------------------------------------------

    /// Queue a job; it starts when a download slot is free.
    fn start_job(&mut self, key: &str, kind: JobKind) {
        self.start_job_in(key, kind, None);
    }

    fn start_job_in(&mut self, key: &str, kind: JobKind, location: Option<PathBuf>) {
        if self.jobs.contains_key(key) {
            return;
        }
        let label = self.app_name(key);
        let mut job = Job::new(kind, label);
        job.location = location;
        self.jobs.insert(key.to_string(), job);
        self.job_order.push(key.to_string());
        self.pump_queue();
    }

    fn pump_queue(&mut self) {
        let limit = self.settings.max_parallel_downloads.max(1) as usize;
        let mut active = self.jobs.values().filter(|j| !j.queued).count();
        for key in self.job_order.clone() {
            if active >= limit {
                break;
            }
            if let Some(job) = self.jobs.get_mut(&key).filter(|j| j.queued) {
                job.queued = false;
                worker::start(&self.bus, &self.manager, &key, job);
                active += 1;
            }
        }
    }

    /// Start a job, asking first if it would change an app that is open.
    fn request(&mut self, id: &str, kind: JobKind) {
        if kind.touches_app() && self.manager.is_running(id) {
            self.running_prompt = Some((id.to_string(), kind));
        } else {
            self.start_job(id, kind);
        }
    }

    /// Like [`Self::request`] but without a dialog: open apps are updated once they close.
    fn request_quietly(&mut self, id: &str, kind: JobKind) {
        if kind.touches_app() && self.manager.is_running(id) {
            if !self.waiting_for_close.iter().any(|(k, _)| k == id) {
                self.toast(ToastKind::Info, format!("{} is open; it will update when you close it", self.app_name(id)));
                self.waiting_for_close.push((id.to_string(), kind));
            }
        } else {
            self.start_job(id, kind);
        }
    }

    fn check_waiting(&mut self) {
        if self.waiting_for_close.is_empty() || self.last_close_check.elapsed() < Duration::from_secs(4) {
            return;
        }
        self.last_close_check = Instant::now();
        let (ready, still): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.waiting_for_close).into_iter().partition(|(id, _)| !self.manager.is_running(id));
        self.waiting_for_close = still;
        for (id, kind) in ready {
            self.start_job(&id, kind);
        }
    }

    /// Fetch an app's releases if nothing is known about them yet.
    pub fn ensure_releases(&mut self, id: &str) {
        let known = self.state(id).is_some_and(|s| s.known);
        if !known && !self.releases_failed.contains_key(id) && self.releases_loading.insert(id.to_string()) {
            worker::fetch_releases(&self.bus, &self.manager, id);
        }
    }

    pub fn ensure_readme(&mut self, id: &str) {
        if !self.readmes.contains_key(id) && self.readme_loading.insert(id.to_string()) {
            worker::fetch_readme(&self.bus, &self.manager, id);
        }
    }

    pub fn ensure_tour(&mut self, id: &str) {
        if !self.tours.contains_key(id) && self.tour_loading.insert(id.to_string()) {
            worker::fetch_tour(&self.bus, &self.manager, id);
        }
    }

    pub fn ensure_shot(&mut self, url: &str) {
        if !self.shots.contains_key(url) && self.shots_loading.insert(url.to_string()) {
            worker::fetch_shot(&self.bus, &self.manager, url);
        }
    }

    fn save_settings(&mut self, settings: Settings) {
        let refetch = settings.include_prereleases != self.settings.include_prereleases
            || settings.prefer_system_installer != self.settings.prefer_system_installer
            || settings.github_token != self.settings.github_token;
        let autostart = settings.start_at_login != self.settings.start_at_login;
        match self.manager.set_settings(settings) {
            Ok(()) => {
                self.settings = self.manager.settings();
                self.reload_states();
                if refetch {
                    self.start_refresh(true, false);
                }
                if autostart {
                    if let Err(err) =
                        oktospace_core::autostart::set(self.settings.start_at_login, &launcher_path(&self.manager))
                    {
                        self.toast(ToastKind::Error, format!("Couldn't change start at login: {err:#}"));
                    }
                }
                self.pump_queue();
            }
            Err(err) => self.toast(ToastKind::Error, format!("Couldn't save settings: {err:#}")),
        }
    }

    /// Whether to ask "portable or installer?" before this action (once, before the first
    /// install, where there's a choice).
    fn ask_install_mode(&self, action: &Action) -> bool {
        let first_install = match action {
            Action::Install(id, _) | Action::InstallTo(id) => self.manager.installed_app(id).is_none(),
            Action::InstallMany(_) => true,
            _ => false,
        };
        first_install
            && !self.settings.install_mode_chosen
            && !self.manager.policy().locks("prefer_system_installer")
            && views::install_mode::has_choice(&self.manager)
    }

    fn handle_actions(&mut self, ctx: &egui::Context) {
        for action in std::mem::take(&mut self.actions) {
            if self.ask_install_mode(&action) {
                self.install_mode_pending = Some(action);
                continue;
            }
            match action {
                Action::Install(id, version) => {
                    let installed = self.manager.installed_app(&id).is_some();
                    let kind = if installed && version.is_none() { JobKind::Update } else { JobKind::Install(version) };
                    if installed {
                        self.request(&id, kind);
                    } else {
                        self.start_job(&id, kind);
                    }
                }
                Action::InstallTo(id) => {
                    let name = self.app_name(&id);
                    let start = self.settings.install_dir.clone().unwrap_or_else(|| self.manager.apps_root());
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title(format!("Install {name} in"))
                        .set_directory(&start)
                        .pick_folder()
                    {
                        self.toast(ToastKind::Info, format!("Installing {name} in {}", dir.join(&id).display()));
                        self.start_job_in(&id, JobKind::Install(None), Some(dir));
                    }
                }
                Action::InstallMany(ids) => {
                    let n = ids.len();
                    for id in ids {
                        self.start_job(&id, JobKind::Install(None));
                    }
                    self.toast(ToastKind::Info, format!("Installing {n} app{}", if n == 1 { "" } else { "s" }));
                }
                Action::Update(id) => self.request(&id, JobKind::Update),
                Action::UpdateAll => {
                    let ids: Vec<String> =
                        self.states.iter().filter(|s| s.update_available).map(|s| s.app.id.clone()).collect();
                    for id in ids {
                        self.request_quietly(&id, JobKind::Update);
                    }
                }
                Action::AskUninstall(id) => self.confirm_uninstall = Some(id),
                Action::SaveReport => {
                    let report = self.manager.report();
                    let name = format!("oktospace-report-{}.json", report["computer"].as_str().unwrap_or("computer"));
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Save a status report")
                        .set_file_name(name)
                        .add_filter("JSON", &["json"])
                        .save_file()
                    {
                        let report = serde_json::to_string_pretty(&report).unwrap_or_default();
                        match std::fs::write(&path, report) {
                            Ok(()) => self.toast(ToastKind::Success, format!("Saved the report to {}", path.display())),
                            Err(err) => self.toast(ToastKind::Error, format!("Couldn't save the report: {err}")),
                        }
                    }
                }
                Action::FillCache => {
                    self.toast(ToastKind::Info, "Downloading the installed apps into the package cache…");
                    worker::fill_cache(&self.bus, &self.manager);
                }
                Action::Uninstall(id) => self.request(&id, JobKind::Uninstall),
                Action::Rollback(id) => self.request(&id, JobKind::Rollback),
                Action::ToggleFavorite(id) => match self.manager.toggle_favorite(&id) {
                    Ok(now) => {
                        self.settings = self.manager.settings();
                        let name = self.app_name(&id);
                        self.toast(
                            ToastKind::Info,
                            if now {
                                format!("{name} is a favorite: you'll hear about its new releases")
                            } else {
                                format!("{name} isn't a favorite any more")
                            },
                        );
                        if now {
                            self.ensure_releases(&id);
                        }
                    }
                    Err(err) => self.toast(ToastKind::Error, format!("{err:#}")),
                },
                Action::SaveBuildOptions(id, options) => {
                    let mut s = self.settings.clone();
                    if options == Default::default() {
                        s.builds.remove(&id);
                    } else {
                        s.builds.insert(id.clone(), options);
                    }
                    self.save_settings(s);
                    self.toast(ToastKind::Success, format!("Saved how to build {}", self.app_name(&id)));
                }
                Action::SkipVersion(id) => match self.manager.skip_version(&id) {
                    Ok(v) => {
                        self.settings = self.manager.settings();
                        self.reload_states();
                        self.toast(
                            ToastKind::Info,
                            format!("Skipped {} {v}: no update until the release after it", self.app_name(&id)),
                        );
                    }
                    Err(err) => self.toast(ToastKind::Error, format!("{err:#}")),
                },
                Action::Unskip(id) => {
                    if let Err(err) = self.manager.unskip(&id) {
                        self.toast(ToastKind::Error, format!("{err:#}"));
                    }
                    self.settings = self.manager.settings();
                    self.reload_states();
                }
                Action::SetAssetChoice(id, pattern) => match self.manager.set_asset_choice(&id, pattern.as_deref()) {
                    Ok(()) => {
                        self.settings = self.manager.settings();
                        self.reload_states();
                    }
                    Err(err) => self.toast(ToastKind::Error, format!("{err:#}")),
                },
                Action::SaveLaunchOptions(id, options) => match self.manager.set_launch_options(&id, options) {
                    Ok(()) => {
                        self.settings = self.manager.settings();
                        self.toast(ToastKind::Success, format!("Saved how {} starts", self.app_name(&id)));
                    }
                    Err(err) => self.toast(ToastKind::Error, format!("{err:#}")),
                },
                Action::TrimPrevious(id) => match self.manager.remove_previous(&id) {
                    Ok(freed) => {
                        self.reload_states();
                        self.storage = Some(self.manager.storage());
                        self.toast(
                            ToastKind::Success,
                            format!("Freed {}", oktospace_core::download::format_bytes(freed)),
                        );
                    }
                    Err(err) => self.toast(ToastKind::Error, format!("{err:#}")),
                },
                Action::OpenPalette => self.palette_open = Some(Default::default()),
                Action::OpenGithub => self.github = Some(Default::default()),
                Action::SearchGithub(q) => {
                    if let Some(d) = &mut self.github {
                        d.busy = true;
                        d.error = None;
                    }
                    worker::search_github(&self.bus, &self.manager, q, false);
                }
                Action::LoadStars(user) => {
                    if let Some(d) = &mut self.github {
                        d.busy = true;
                        d.error = None;
                    }
                    worker::search_github(&self.bus, &self.manager, user, true);
                }
                Action::AddRepo(repo) => worker::add_repo(&self.bus, &self.manager, repo),
                Action::Build(id) => {
                    self.build_logs.remove(&id);
                    if self.apps_view == AppsView::Detail(id.clone()) {
                        self.detail_tab = DetailTab::Build;
                    }
                    self.request(&id, JobKind::Build(None));
                }
                Action::VerifyAndRepair(id) => {
                    self.toast(ToastKind::Info, format!("Checking {}'s files…", self.app_name(&id)));
                    worker::verify(&self.bus, &self.manager, &id);
                }
                Action::Cancel(key) => {
                    match self.jobs.get(&key) {
                        Some(job) if job.queued => {
                            self.jobs.remove(&key);
                            self.job_order.retain(|k| *k != key);
                        }
                        Some(job) => job.cancel.store(true, Ordering::Relaxed),
                        None => {}
                    }
                    self.waiting_for_close.retain(|(k, _)| *k != key);
                }
                Action::WhenClosed(id, kind) => {
                    self.toast(
                        ToastKind::Info,
                        format!("{} will continue when you close {}", kind.verb(), self.app_name(&id)),
                    );
                    self.waiting_for_close.push((id, kind));
                }
                Action::RunNow(id, kind) => self.start_job(&id, kind),
                Action::Launch(id, files) => match self.manager.launch(&id, &files) {
                    Ok(()) => {
                        let name = self.app_name(&id);
                        self.toast(ToastKind::Info, format!("Opening {name}…"));
                    }
                    Err(err) => self.toast(ToastKind::Error, format!("{err:#}")),
                },
                // Straight to the system's browser, so a failure can be reported.
                Action::OpenUrl(url) => {
                    if let Err(err) = open::that_detached(&url) {
                        self.toast(ToastKind::Error, format!("Couldn't open {url}: {err}"));
                    }
                }
                Action::OpenPath(path) => {
                    if let Err(err) = open::that_detached(&path) {
                        self.toast(ToastKind::Error, format!("Couldn't open {}: {err}", path.display()));
                    }
                }
                Action::ShowApp(id) => {
                    if !matches!(self.apps_view, AppsView::Detail(_)) {
                        self.back_view = self.apps_view.clone();
                    }
                    self.tab = Tab::Apps;
                    self.detail_release = None;
                    self.detail_tab = DetailTab::Overview;
                    self.apps_view = AppsView::Detail(id);
                }
                Action::GoApps(view) => {
                    self.tab = Tab::Apps;
                    self.apps_view = view;
                }
                Action::Refresh => self.start_refresh(true, true),
                Action::OpenSettings => {
                    self.settings_draft = Some(self.settings.clone());
                    self.storage = Some(self.manager.storage());
                    self.toolchains = Some(oktospace_core::build::toolchains());
                }
                Action::SaveSettings(mut s) => {
                    // The Settings window doesn't edit these (its own window does, maybe while
                    // Settings is open), so keep what's saved.
                    // Its dialog changes the lists; Settings only turns them on or off.
                    let enabled = s.other_sources.enabled;
                    s.other_sources = self.settings.other_sources.clone();
                    s.other_sources.enabled = enabled;
                    // Changed on app pages, maybe while Settings was open.
                    let now = &self.settings;
                    (s.favorites, s.builds, s.skipped, s.assets, s.launch, s.channels) = (
                        now.favorites.clone(),
                        now.builds.clone(),
                        now.skipped.clone(),
                        now.assets.clone(),
                        now.launch.clone(),
                        now.channels.clone(),
                    );
                    self.save_settings(*s);
                }
                Action::ApplySelfUpdate => {
                    if let Some(update) = self.self_update.clone() {
                        self.self_updating = true;
                        worker::apply_self_update(&self.bus, &self.manager, update);
                    }
                }
                Action::SelfInstall => {
                    self.offer_self_install = false;
                    match selfupdate::self_install(&self.manager) {
                        Ok(_) => self.toast(
                            ToastKind::Success,
                            "OktoSpace is installed. Find it in your Start menu or app menu.",
                        ),
                        Err(err) => self.toast(ToastKind::Error, format!("Couldn't install OktoSpace: {err:#}")),
                    }
                }
                Action::DismissSelfInstall => {
                    self.offer_self_install = false;
                    let s = Settings { offer_self_install: false, ..self.settings.clone() };
                    self.save_settings(s);
                }
                Action::TestNotification => worker::test_notification(&self.bus),
                Action::OpenAbout => self.about_open = true,
                Action::OpenSource(target) => {
                    let current = match &target {
                        views::sources::Target::App(id) if id == selfupdate::NAME => {
                            self.settings.other_sources.self_repo.clone().unwrap_or_default()
                        }
                        views::sources::Target::App(id) => {
                            self.settings.other_sources.overrides.get(id).cloned().unwrap_or_default()
                        }
                        views::sources::Target::NewApp => String::new(),
                    };
                    self.source_dialog = Some(views::sources::Dialog::new(target, current));
                }
                Action::Source(op) => worker::source_op(&self.bus, &self.manager, op),
                Action::RestartOktoSpace => {
                    let mut cmd = std::process::Command::new(launcher_path(&self.manager));
                    if self.hidden {
                        cmd.arg(oktospace_core::autostart::BACKGROUND_FLAG);
                    }
                    match cmd.spawn() {
                        Ok(_) => {
                            self.quitting = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(err) => self.toast(ToastKind::Error, format!("Couldn't restart OktoSpace: {err}")),
                    }
                }
                Action::SetInstallMode(installer) => {
                    let mut s = self.settings.clone();
                    s.prefer_system_installer = installer;
                    s.install_mode_chosen = true;
                    self.save_settings(s);
                    if let Some(next) = self.install_mode_pending.take() {
                        self.actions.push(next);
                    }
                }
                Action::Cleanup => {
                    match self.manager.cleanup() {
                        Ok(freed) => self.toast(
                            ToastKind::Success,
                            format!("Freed {}", oktospace_core::download::format_bytes(freed)),
                        ),
                        Err(err) => self.toast(ToastKind::Error, format!("{err:#}")),
                    }
                    if self.settings_draft.is_some() {
                        self.storage = Some(self.manager.storage());
                    }
                }
                Action::SetChannel(id, channel) => {
                    let mut s = self.settings.clone();
                    let commits = channel == Channel::Commits;
                    if channel == Channel::Default {
                        s.channels.remove(&id);
                    } else {
                        s.channels.insert(id.clone(), channel);
                    }
                    self.save_settings(s);
                    // Following commits: look up the newest one now.
                    if commits && self.releases_loading.insert(id.clone()) {
                        worker::fetch_releases(&self.bus, &self.manager, &id);
                    }
                }
                Action::ReportBug(id) => {
                    if let Some(url) = self.manager.bug_report_url(&id) {
                        ctx.open_url(egui::OpenUrl::new_tab(url));
                    }
                }
                Action::ExportList => {
                    let list = self.manager.export_list();
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Export app list")
                        .set_file_name("oktospace-apps.json")
                        .save_file()
                    {
                        let result = serde_json::to_vec_pretty(&list)
                            .map_err(anyhow::Error::from)
                            .and_then(|j| Ok(std::fs::write(&path, j)?));
                        match result {
                            Ok(()) => self.toast(
                                ToastKind::Success,
                                format!("Saved {} apps to {}", list.apps.len(), path.display()),
                            ),
                            Err(err) => self.toast(ToastKind::Error, format!("Couldn't save: {err:#}")),
                        }
                    }
                }
                Action::ImportList => {
                    let Some(path) = rfd::FileDialog::new()
                        .set_title("Import app list")
                        .add_filter("App list", &["json"])
                        .pick_file()
                    else {
                        continue;
                    };
                    let parsed = std::fs::read(&path)
                        .map_err(anyhow::Error::from)
                        .and_then(|b| serde_json::from_slice::<AppList>(&b).map_err(Into::into))
                        .and_then(|list| self.manager.import_list(&list, false));
                    match parsed {
                        Ok(todo) if todo.is_empty() => {
                            self.toast(ToastKind::Info, "Everything in that list is already installed")
                        }
                        Ok(todo) => {
                            self.settings = self.manager.settings();
                            let ids = todo.into_iter().map(|(id, _)| id).collect();
                            self.actions.push(Action::InstallMany(ids));
                        }
                        Err(err) => {
                            self.toast(ToastKind::Error, format!("Couldn't import {}: {err:#}", path.display()))
                        }
                    }
                }
                Action::OpenMultiInstall => self.multi_install = Some(BTreeSet::new()),
            }
        }
        // Actions queued while handling (import → install).
        if !self.actions.is_empty() {
            self.handle_actions(ctx);
        }
    }

    fn handle_tray(&mut self, ctx: &egui::Context) {
        while let Ok(cmd) = self.tray_rx.try_recv() {
            match cmd {
                TrayCommand::Show => {
                    log::info!("opened from the tray");
                    self.hidden = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayCommand::CheckUpdates => self.start_refresh(true, true),
                TrayCommand::UpdateAll => self.actions.push(Action::UpdateAll),
                TrayCommand::Quit => {
                    for job in self.jobs.values() {
                        job.cancel.store(true, Ordering::Relaxed);
                    }
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        ui.horizontal_centered(|ui| {
            ui.add_space(6.0);
            let (rect, logo) = ui.allocate_exact_size(Vec2::splat(30.0), egui::Sense::click());
            egui::Image::new(&theme::logo(ui.ctx())).paint_at(ui, rect);
            let name =
                ui.add(egui::Label::new(RichText::new("OktoSpace").size(17.0).strong()).sense(egui::Sense::click()));
            if (logo | name).on_hover_text("App library").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.search.clear();
                self.actions.push(Action::GoApps(AppsView::All));
            }
            ui.add_space(18.0);
            for (tab, label) in [(Tab::Apps, "Apps"), (Tab::Discover, "Discover")] {
                if theme::tab(ui, &p, label, self.tab == tab).clicked() {
                    self.tab = tab;
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(6.0);
                // The menu: settings, about, update check.
                let menu = theme::icon_button(ui, &p, theme::Icon::Glyph("⚙"), 30.0).on_hover_text("Menu");
                egui::Popup::menu(&menu).show(|ui| {
                    ui.set_min_width(190.0);
                    if ui.button("Settings…").clicked() {
                        self.actions.push(Action::OpenSettings);
                    }
                    if ui.add_enabled(!self.refreshing, egui::Button::new("Check for updates")).clicked() {
                        self.actions.push(Action::Refresh);
                    }
                    ui.separator();
                    if ui.button("About OktoSpace").clicked() {
                        self.actions.push(Action::OpenAbout);
                    }
                });
                if self.refreshing {
                    ui.add(egui::Spinner::new().size(14.0));
                } else if theme::icon_button(ui, &p, theme::Icon::Glyph("⟳"), 30.0)
                    .on_hover_text("Check for updates")
                    .clicked()
                {
                    self.actions.push(Action::Refresh);
                }
                let updates = self.update_count();
                let bell = theme::icon_button(ui, &p, theme::Icon::Bell, 30.0);
                if updates > 0 {
                    let c = bell.rect.right_top() + Vec2::new(-3.0, 4.0);
                    ui.painter().circle_filled(c, 7.0, p.bad);
                    ui.painter().text(
                        c,
                        egui::Align2::CENTER_CENTER,
                        updates.to_string(),
                        egui::FontId::proportional(10.0),
                        Color32::WHITE,
                    );
                }
                if bell
                    .on_hover_text(if updates > 0 {
                        format!("{updates} update(s) available")
                    } else {
                        "No updates".into()
                    })
                    .clicked()
                {
                    self.actions.push(Action::GoApps(AppsView::Updates));
                }
                if !self.jobs.is_empty() || !self.waiting_for_close.is_empty() {
                    views::downloads::button(self, ui);
                }
                if self.manager.policy().is_active() {
                    self.managed_chip(ui);
                }
                ui.add_space(12.0);
                let hint = "Search apps";
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(format!("🔍  {hint}"))
                        .desired_width(ui.available_width().clamp(120.0, 340.0))
                        .margin(Margin::symmetric(10, 6)),
                );
                let keys = if cfg!(target_os = "macos") { "⌘K" } else { "Ctrl+K" };
                if ui
                    .add(egui::Button::new(RichText::new(keys).size(12.0).color(self.palette.weak)).frame(false))
                    .on_hover_text("Commands: jump to an app, install, update, open, or any command")
                    .clicked()
                {
                    self.actions.push(Action::OpenPalette);
                }
            });
        });
    }

    /// "Managed by …" in the top bar: what the organization decides, and where to get help.
    fn managed_chip(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let policy = self.manager.policy().clone();
        let text = match &policy.organization {
            Some(org) => format!("🏫 {org}"),
            None => "🏫 Managed".into(),
        };
        let chip = ui
            .add(
                egui::Button::new(RichText::new(text).size(12.0).color(p.warn))
                    .fill(p.warn.gamma_multiply(0.15))
                    .corner_radius(9.0),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let mut lines = vec![format!("{} manages OktoSpace on this computer.", self.organization())];
        if policy.lock_settings {
            lines.push("Settings can't be changed.".into());
        } else if !policy.settings.is_empty() {
            lines.push(format!("{} setting(s) are set for you.", policy.settings.len()));
        }
        if !policy.required_apps.is_empty() {
            lines.push(format!("Always installed: {}.", self.names(&policy.required_apps)));
        }
        if !policy.pinned_versions.is_empty() {
            let pins: Vec<String> =
                policy.pinned_versions.iter().map(|(id, v)| format!("{} {v}", self.app_name(id))).collect();
            lines.push(format!("Kept on: {}.", pins.join(", ")));
        }
        if !policy.blocked_apps.is_empty() {
            lines.push(format!("Not available: {}.", self.names(&policy.blocked_apps)));
        }
        if let Some(w) = &policy.update_window {
            lines.push(format!("Updates install {}.", w.describe()));
        }
        if policy.prevent_uninstall {
            lines.push("Apps can't be uninstalled or rolled back.".into());
        }
        if let Some(support) = &policy.support {
            lines.push(format!("Help: {support}"));
        }
        let chip = chip.on_hover_text(lines.join("\n"));
        if chip.clicked() {
            match policy.support.as_deref() {
                Some(s) if s.starts_with("http://") || s.starts_with("https://") => {
                    self.actions.push(Action::OpenUrl(s.to_string()))
                }
                Some(s) if s.contains('@') && !s.contains(' ') => {
                    self.actions.push(Action::OpenUrl(format!("mailto:{s}")))
                }
                _ => self.actions.push(Action::OpenSettings),
            }
        }
    }

    fn names(&self, ids: &[String]) -> String {
        ids.iter().map(|id| self.app_name(id)).collect::<Vec<_>>().join(", ")
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        let p = self.palette;
        self.toasts.retain(|t| t.at.elapsed() < Duration::from_secs(if t.kind == ToastKind::Error { 9 } else { 5 }));
        if self.toasts.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -16.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let mut dismiss = None;
                for (i, t) in self.toasts.iter().enumerate() {
                    let color = match t.kind {
                        ToastKind::Info => p.accent,
                        ToastKind::Success => p.good,
                        ToastKind::Error => p.bad,
                    };
                    Frame::new()
                        .fill(p.panel)
                        .stroke(Stroke::new(1.0, p.stroke))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(14, 10))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 16,
                            spread: 0,
                            color: Color32::from_black_alpha(80),
                        })
                        .show(ui, |ui| {
                            ui.set_max_width(380.0);
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(Vec2::new(4.0, 18.0), egui::Sense::hover());
                                ui.painter().rect_filled(r, CornerRadius::same(2), color);
                                ui.add(egui::Label::new(&t.text).wrap());
                                if ui.add(egui::Button::new(RichText::new("×").color(p.weak)).frame(false)).clicked() {
                                    dismiss = Some(i);
                                }
                            });
                        });
                    ui.add_space(6.0);
                }
                if let Some(i) = dismiss {
                    self.toasts.remove(i);
                }
            });
    }

    fn modals(&mut self, ctx: &egui::Context) {
        let p = self.palette;
        if let Some(id) = self.confirm_uninstall.clone() {
            let name = self.app_name(&id);
            let modal = egui::Modal::new(egui::Id::new("confirm-uninstall")).show(ctx, |ui| {
                ui.set_width(400.0);
                ui.heading(format!("Uninstall {name}?"));
                ui.add_space(6.0);
                ui.label(RichText::new("The app, its shortcuts and any version kept for rollback are removed. Your documents and app preferences are kept.").color(p.weak));
                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if danger_button(ui, &p, "Uninstall").clicked() {
                        self.actions.push(Action::Uninstall(id.clone()));
                        ui.close();
                    }
                    if theme::pill(ui, &p, "Cancel").clicked() {
                        ui.close();
                    }
                });
            });
            if modal.should_close() {
                self.confirm_uninstall = None;
            }
        }

        if let Some((id, kind)) = self.running_prompt.clone() {
            let name = self.app_name(&id);
            let managed = self.manager.installed_app(&id).is_some_and(|i| i.current.kind.is_managed());
            let modal = egui::Modal::new(egui::Id::new("app-running")).show(ctx, |ui| {
                ui.set_width(440.0);
                ui.heading(format!("{name} is open"));
                ui.add_space(6.0);
                let explain = match (&kind, managed) {
                    (JobKind::Uninstall, _) => format!("Close {name} before uninstalling it, or let OktoSpace do it as soon as you close it."),
                    (_, true) => format!("You can go ahead: the new version installs next to the open one and is used the next time you start {name}. Or wait until you close it."),
                    (_, false) => format!("{name}'s installer needs it to be closed. OktoSpace can continue as soon as you close it."),
                };
                ui.label(RichText::new(explain).color(p.weak));
                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if managed && kind != JobKind::Uninstall && theme::primary(ui, &p, &format!("{} now", short_verb(&kind))).clicked() {
                        self.actions.push(Action::RunNow(id.clone(), kind.clone()));
                        ui.close();
                    }
                    if theme::pill(ui, &p, "When it closes").clicked() {
                        self.actions.push(Action::WhenClosed(id.clone(), kind.clone()));
                        ui.close();
                    }
                    if ui.add(egui::Button::new("Cancel").frame(false)).clicked() {
                        ui.close();
                    }
                });
            });
            if modal.should_close() {
                self.running_prompt = None;
            }
        }

        if self.multi_install.is_some() {
            views::apps::multi_install_modal(self, ctx);
        }

        if self.confirm_quit {
            let modal = egui::Modal::new(egui::Id::new("confirm-quit")).show(ctx, |ui| {
                ui.set_width(360.0);
                ui.heading("Quit while apps are installing?");
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Installs in progress will be cancelled. Partial downloads resume next time.")
                        .color(p.weak),
                );
                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if danger_button(ui, &p, "Quit").clicked() {
                        for job in self.jobs.values() {
                            job.cancel.store(true, Ordering::Relaxed);
                        }
                        self.jobs.clear();
                        self.quitting = true;
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        ui.close();
                    }
                    if theme::pill(ui, &p, "Keep going").clicked() {
                        ui.close();
                    }
                });
            });
            if modal.should_close() {
                self.confirm_quit = false;
            }
        }

        views::settings::window(self, ctx);
        views::about::window(self, ctx);
        views::sources::window(self, ctx);
        views::github::window(self, ctx);
        views::palette::window(self, ctx);
        views::install_mode::window(self, ctx);
    }
}

fn short_verb(kind: &JobKind) -> &'static str {
    match kind {
        JobKind::Update => "Update",
        JobKind::Rollback => "Roll back",
        JobKind::Repair => "Repair",
        JobKind::Build(_) => "Build",
        _ => "Continue",
    }
}

pub fn danger_button(ui: &mut egui::Ui, p: &Palette, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE)).fill(p.bad).corner_radius(CornerRadius::same(15)),
    )
}

impl eframe::App for OktoSpaceApp {
    /// Everything that must keep happening while the window is hidden in the tray.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_messages(ctx);
        self.handle_tray(ctx);
        self.dock.update(self.settings.dock_icon, !self.hidden);
        // Back in front: a window minimized instead of hidden (Wayland) is open again, and the
        // notification about what happened meanwhile has been seen.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
        if focused && !self.was_focused {
            if self.hidden && !can_hide_windows() {
                self.hidden = false;
            }
            worker::withdraw_notifications();
        }
        self.was_focused = focused;
        self.keep_findable(ctx);
        if self.settings.auto_check_updates {
            let every = Duration::from_secs(u64::from(self.settings.check_interval_hours.max(1)) * 3600);
            if self.last_check.is_none_or(|t| t.elapsed() >= every) {
                self.start_refresh(false, false);
            }
        }
        self.check_waiting();
        if !self.actions.is_empty() {
            self.handle_actions(ctx);
        }
        if !self.jobs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Keyboard: Ctrl/⌘+K the command palette, Ctrl/⌘+R check for updates.
        use egui::{Key, KeyboardShortcut, Modifiers};
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::K))) {
            self.palette_open = if self.palette_open.is_some() { None } else { Some(Default::default()) };
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::R))) {
            self.actions.push(Action::Refresh);
        }

        // Theme.
        let system_light = ctx.system_theme() == Some(egui::Theme::Light);
        if self.applied_theme != Some((self.settings.theme, system_light)) {
            self.palette = theme::palette_for(self.settings.theme, &ctx);
            theme::apply(&ctx, &self.palette);
            self.applied_theme = Some((self.settings.theme, system_light));
        }

        // Closing: confirm during installs, else hide to the tray if that's on.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            if !self.jobs.is_empty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm_quit = true;
            } else if self.settings.keep_running_in_tray && self.tray_shown() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.put_away(&ctx);
                log::info!("window closed; still running in the tray");
                if !self.told_about_tray {
                    self.told_about_tray = true;
                    worker::notify_transient(if can_hide_windows() {
                        "OktoSpace keeps checking for updates in the background. Quit it from the tray icon.".into()
                    } else {
                        "OktoSpace is minimized and keeps checking for updates. Quit it from the tray icon (or turn off \"Keep running in the tray\" in Settings).".into()
                    });
                }
            }
        }

        let p = self.palette;
        egui::Panel::top("top")
            .exact_size(56.0)
            .frame(Frame::new().fill(p.panel).inner_margin(Margin::symmetric(12, 0)).stroke(Stroke::new(1.0, p.stroke)))
            .show(ui, |ui| self.top_bar(ui));

        if self.tab != Tab::Discover {
            egui::Panel::left("sidebar")
                .exact_size(232.0)
                .resizable(false)
                .frame(
                    Frame::new()
                        .fill(p.panel)
                        .inner_margin(Margin::symmetric(14, 12))
                        .stroke(Stroke::new(1.0, p.stroke)),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                        Tab::Apps => views::apps::sidebar(self, ui),
                        Tab::Discover => {}
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 28, right: 20, top: 20, bottom: 0 }))
            .show(ui, |ui| match self.tab {
                Tab::Apps => views::apps::content(self, ui),
                Tab::Discover => views::discover::content(self, ui),
            });

        if self.downloads_open {
            views::downloads::panel(self, &ctx);
        }
        self.modals(&ctx);
        self.toasts(&ctx);
        self.handle_actions(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for job in self.jobs.values() {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// Whether the window can be hidden (and shown again from the tray). Wayland compositors don't
/// let a program hide or raise its own window, so there it's minimized to the taskbar instead.
fn can_hide_windows() -> bool {
    !(cfg!(all(unix, not(target_os = "macos"))) && std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty()))
}

/// What start-at-login should run: the installed copy when there is one.
fn launcher_path(manager: &Manager) -> PathBuf {
    let current = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("oktospace"));
    // An AppImage's programs only exist while it runs: start the AppImage itself.
    if let Some(appimage) = selfupdate::running_appimage() {
        let installed = selfupdate::installed_appimage(manager);
        return if installed.is_file() { installed } else { appimage };
    }
    if let Some(bundle) = selfupdate::running_bundle() {
        return bundle.join("Contents/MacOS").join(current.file_name().unwrap_or_default());
    }
    let installed = selfupdate::install_dir(manager).join("bin").join(current.file_name().unwrap_or_default());
    if installed.is_file() {
        installed
    } else {
        current
    }
}

pub fn version_string() -> String {
    selfupdate::current_version().to_string()
}
