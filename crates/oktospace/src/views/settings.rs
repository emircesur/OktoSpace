//! The Settings window. Edits a draft copy that is saved on "Save". Settings a machine policy
//! sets are shown but can't be changed.

use eframe::egui::{self, Align, Layout, RichText, Ui};
use oktospace_core::platform::Os;
use oktospace_core::policy::Policy;
use oktospace_core::settings::{Settings, Theme};

use crate::app::{Action, OktoSpaceApp};
use crate::theme;

/// The Settings window's pages, listed down its side menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    General,
    Downloads,
    Installation,
    Appearance,
    GitHubApps,
    Storage,
    BuildTools,
    GitHub,
    YourApps,
    It,
    About,
}

impl Page {
    const ALL: [Page; 11] = [
        Page::General,
        Page::Downloads,
        Page::Installation,
        Page::Appearance,
        Page::GitHubApps,
        Page::Storage,
        Page::BuildTools,
        Page::GitHub,
        Page::YourApps,
        Page::It,
        Page::About,
    ];

    fn label(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Downloads => "Downloads",
            Page::Installation => "Installation",
            Page::Appearance => "Appearance",
            Page::GitHubApps => "Apps from GitHub",
            Page::Storage => "Storage",
            Page::BuildTools => "Build tools",
            Page::GitHub => "GitHub account",
            Page::YourApps => "Export & import",
            Page::It => "IT & Classroom",
            Page::About => "About",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Page::General => "How OktoSpace checks for updates and keeps itself running.",
            Page::Downloads => "How apps are downloaded.",
            Page::Installation => "Where and how apps are installed.",
            Page::Appearance => "How OktoSpace looks.",
            Page::GitHubApps => "Apps from any GitHub repository, and forks to update from.",
            Page::Storage => "How much space apps, downloads and builds take, and freeing it.",
            Page::BuildTools => "What building from source can use on this computer.",
            Page::GitHub => "Release information comes from GitHub.",
            Page::YourApps => "Set up another computer with the same apps.",
            Page::It => "Policies, a shared package cache and status reports for labs and classrooms.",
            Page::About => "Version, data folder and source code.",
        }
    }
}

pub fn window(app: &mut OktoSpaceApp, ctx: &egui::Context) {
    let Some(mut draft) = app.settings_draft.take() else { return };
    let p = app.palette;
    let mut keep_open = true;
    let mut save = false;
    let policy = app.manager.policy().clone();
    let height = (ctx.content_rect().height() - 200.0).clamp(320.0, 580.0);
    let width = (ctx.content_rect().width() - 80.0).clamp(560.0, 860.0);

    let modal = egui::Modal::new(egui::Id::new("settings")).show(ctx, |ui| {
        ui.set_width(width);
        ui.horizontal(|ui| {
            ui.heading("Settings");
            if policy.is_active() {
                let org = policy.organization.as_deref().unwrap_or("your organization");
                let text = if policy.lock_settings {
                    format!("Managed by {org}: settings can't be changed here")
                } else {
                    format!("Some settings are managed by {org}")
                };
                ui.add_space(8.0);
                theme::chip(ui, &text, p.warn);
            }
        });
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            // The side menu.
            ui.allocate_ui_with_layout(egui::vec2(180.0, height), Layout::top_down(Align::Min), |ui| {
                ui.set_min_size(egui::vec2(180.0, height));
                for page in Page::ALL {
                    if nav_item(ui, &p, page.label(), app.settings_page == page) {
                        app.settings_page = page;
                    }
                }
            });
            let (line, _) = ui.allocate_exact_size(egui::vec2(9.0, height), egui::Sense::hover());
            ui.painter().vline(line.center().x, line.y_range(), egui::Stroke::new(1.0, p.stroke));
            // The page.
            ui.vertical(|ui| {
                ui.set_height(height);
                let page = app.settings_page;
                ui.label(RichText::new(page.label()).size(19.0).strong());
                ui.label(RichText::new(page.blurb()).size(12.5).color(p.weak));
                ui.add_space(10.0);
                egui::ScrollArea::vertical()
                    .id_salt(("settings-page", page as u8))
                    .max_height(height - 50.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        match page {
                            Page::General => general(app, ui, &p, &policy, &mut draft),
                            Page::Downloads => downloads(ui, &p, &policy, &mut draft),
                            Page::Installation => installation(app, ui, &p, &policy, &mut draft),
                            Page::Appearance => appearance(app, ui, &p, &mut draft),
                            Page::GitHubApps => github_apps(app, ui, &p, &policy, &mut draft),
                            Page::Storage => group(ui, &p, |ui| storage_section(app, ui, &p)),
                            Page::BuildTools => build_tools(app, ui, &p),
                            Page::GitHub => github(ui, &p, &policy, &mut draft),
                            Page::YourApps => your_apps(app, ui, &p),
                            Page::It => group(ui, &p, |ui| it_section(app, ui, &p, &policy, &mut draft)),
                            Page::About => about(app, ui, &p, &policy),
                        }
                    });
            });
        });
        ui.add_space(8.0);
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::primary(ui, &p, "Save").clicked() {
                save = true;
                keep_open = false;
            }
            if theme::pill(ui, &p, "Cancel").clicked() {
                keep_open = false;
            }
            if draft != app.settings {
                ui.label(RichText::new("Unsaved changes").size(12.0).color(p.weak));
            }
        });
    });
    if modal.should_close() {
        keep_open = false;
    }
    if save {
        app.actions.push(Action::SaveSettings(Box::new(draft.clone())));
    }
    if keep_open {
        app.settings_draft = Some(draft);
    }
}

// ---- building blocks ----------------------------------------------------------------------

/// One entry of the side menu; true when clicked.
fn nav_item(ui: &mut Ui, p: &theme::Palette, label: &str, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 6, if selected { p.card_hover } else { p.card });
    }
    if selected {
        let bar = egui::Rect::from_min_size(rect.left_top() + egui::vec2(0.0, 7.0), egui::vec2(3.0, 16.0));
        ui.painter().rect_filled(bar, 2, p.accent);
    }
    ui.painter().text(
        rect.left_center() + egui::vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        if selected { p.text } else { p.weak },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// A card holding related settings.
fn group<R>(ui: &mut Ui, p: &theme::Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    let r = theme::card_frame(p, false)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner;
    ui.add_space(8.0);
    r
}

/// A heading above a group.
fn group_title(ui: &mut Ui, p: &theme::Palette, title: &str) {
    ui.label(RichText::new(title).size(13.0).strong().color(p.weak));
}

/// A setting: its name and what it does on the left, its control on the right.
fn row(ui: &mut Ui, p: &theme::Palette, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let text_width = (ui.available_width() - 210.0).max(160.0);
        ui.vertical(|ui| {
            ui.set_width(text_width);
            ui.label(RichText::new(title).size(14.0).color(p.text));
            if !desc.is_empty() {
                ui.add(egui::Label::new(RichText::new(desc).size(12.0).color(p.weak)).wrap());
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
}

/// A switch setting.
fn switch_row(ui: &mut Ui, p: &theme::Palette, enabled: bool, value: &mut bool, title: &str, desc: &str) -> bool {
    let mut changed = false;
    row(ui, p, title, desc, |ui| {
        ui.add_enabled_ui(enabled, |ui| changed = toggle(ui, p, value).changed());
    });
    changed
}

/// A thin line between rows of a group.
fn divider(ui: &mut Ui, p: &theme::Palette) {
    let rect = ui.available_rect_before_wrap();
    let y = rect.top() + 2.0;
    ui.painter().hline(rect.x_range(), y, egui::Stroke::new(1.0, p.stroke));
    ui.add_space(4.0);
}

/// An on/off switch.
fn toggle(ui: &mut Ui, p: &theme::Palette, on: &mut bool) -> egui::Response {
    let size = egui::vec2(40.0, 22.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let enabled = ui.is_enabled();
        let off_fill = p.stroke;
        let fill = if *on { p.accent } else { off_fill };
        let fill = if enabled { fill } else { fill.gamma_multiply(0.45) };
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, fill);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        let knob = if *on { p.accent_text } else { p.text };
        ui.painter().circle_filled(
            egui::pos2(x, rect.center().y),
            radius - 4.0,
            if enabled { knob } else { knob.gamma_multiply(0.5) },
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---- pages -----------------------------------------------------------------------------------

fn general(app: &mut OktoSpaceApp, ui: &mut Ui, p: &theme::Palette, policy: &Policy, d: &mut Settings) {
    let free = |key: &str| !policy.locks(key);
    group_title(ui, p, "Updates");
    group(ui, p, |ui| {
        switch_row(
            ui,
            p,
            free("auto_check_updates"),
            &mut d.auto_check_updates,
            "Check for updates automatically",
            "On start and every few hours, from the tray too.",
        );
        if d.auto_check_updates {
            divider(ui, p);
            row(ui, p, "How often", "Hours between checks.", |ui| {
                ui.add_enabled(
                    free("check_interval_hours"),
                    egui::DragValue::new(&mut d.check_interval_hours).range(1..=168).suffix(" h"),
                );
            });
            divider(ui, p);
            switch_row(
                ui,
                p,
                free("auto_install_updates"),
                &mut d.auto_install_updates,
                "Install updates automatically",
                "As soon as they're found. Open apps update when you close them.",
            );
        }
        divider(ui, p);
        switch_row(
            ui,
            p,
            free("include_prereleases"),
            &mut d.include_prereleases,
            "Offer pre-releases",
            "Betas and release candidates. Each app can follow its own channel on its page.",
        );
        divider(ui, p);
        switch_row(
            ui,
            p,
            free("keep_previous_version"),
            &mut d.keep_previous_version,
            "Keep the previous version",
            "So an update can be rolled back. Storage shows the space it takes.",
        );
    });
    group_title(ui, p, "OktoSpace");
    group(ui, p, |ui| {
        switch_row(
            ui,
            p,
            free("auto_update_self"),
            &mut d.auto_update_self,
            "Update OktoSpace automatically",
            "The new version runs from the next start.",
        );
        divider(ui, p);
        row(ui, p, "Notifications", "When updates are found or installed.", |ui| {
            ui.add_enabled_ui(free("notifications"), |ui| {
                toggle(ui, p, &mut d.notifications);
            });
            if ui.small_button("Send a test").clicked() {
                app.actions.push(Action::TestNotification);
            }
        });
        divider(ui, p);
        switch_row(
            ui,
            p,
            free("keep_running_in_tray"),
            &mut d.keep_running_in_tray,
            "Keep running in the tray",
            "When the window is closed, so updates are still found.",
        );
        divider(ui, p);
        switch_row(
            ui,
            p,
            free("start_at_login"),
            &mut d.start_at_login,
            "Start when I log in",
            "In the background, in the tray.",
        );
    });
}

fn downloads(ui: &mut Ui, p: &theme::Palette, policy: &Policy, d: &mut Settings) {
    let free = |key: &str| !policy.locks(key);
    group(ui, p, |ui| {
        row(ui, p, "Apps at a time", "How many apps download and install at once.", |ui| {
            ui.add_enabled(
                free("max_parallel_downloads"),
                egui::DragValue::new(&mut d.max_parallel_downloads).range(1..=6),
            );
        });
        divider(ui, p);
        let mut limited = d.download_limit_kbps.is_some();
        let mut kbps = d.download_limit_kbps.unwrap_or(2048);
        row(
            ui,
            p,
            "Limit download speed",
            "Leave room for everything else on the connection. Interrupted downloads continue where they stopped.",
            |ui| {
                ui.add_enabled_ui(free("download_limit_kbps"), |ui| {
                    toggle(ui, p, &mut limited);
                    ui.add_enabled(
                        limited,
                        egui::DragValue::new(&mut kbps).range(64..=1_000_000).suffix(" KB/s").speed(16),
                    );
                });
            },
        );
        d.download_limit_kbps = limited.then_some(kbps);
        divider(ui, p);
        switch_row(
            ui,
            p,
            free("allow_unverified_downloads"),
            &mut d.allow_unverified_downloads,
            "Allow downloads without a checksum",
            "Not recommended: OktoSpace can't check these files weren't changed.",
        );
    });
}

fn installation(app: &mut OktoSpaceApp, ui: &mut Ui, p: &theme::Palette, policy: &Policy, d: &mut Settings) {
    let free = |key: &str| !policy.locks(key);
    let os = app.manager.platform().os;
    group(ui, p, |ui| {
        let current = d.install_dir.clone().unwrap_or_else(|| app.manager.paths().apps.clone());
        row(ui, p, "Install apps in", &current.display().to_string(), |ui| {
            ui.add_enabled_ui(free("install_dir"), |ui| {
                if d.install_dir.is_some() && ui.button("Default").clicked() {
                    d.install_dir = None;
                }
                if ui.button("Change…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().set_title("Install apps in").pick_folder() {
                        d.install_dir = Some(dir);
                    }
                }
            });
        });
        ui.label(RichText::new("Installed apps stay where they are. For one app elsewhere, use \"Install in a folder…\" in its ••• menu.").size(12.0).color(p.weak));
        if os == Os::Linux && d.prefer_system_installer && oktospace_core::platform::linux_package_format().is_some() {
            ui.label(
                RichText::new("System packages go where the package manager puts them (/usr).")
                    .size(12.0)
                    .color(p.warn),
            );
        }
        divider(ui, p);
        switch_row(
            ui,
            p,
            free("detect_installed"),
            &mut d.detect_installed,
            "Find apps installed without OktoSpace",
            "From their own installers, disk images, packages or Flatpak; they're updated where they are.",
        );
        if os != Os::Macos {
            divider(ui, p);
            switch_row(
                ui,
                p,
                free("desktop_shortcuts"),
                &mut d.desktop_shortcuts,
                "Desktop shortcuts",
                "Besides the menu entry.",
            );
        }
        match os {
            Os::Windows => {
                divider(ui, p);
                if switch_row(ui, p, free("prefer_system_installer"), &mut d.prefer_system_installer, "Use each app's installer", "MSI or Setup instead of the portable build. Installers may ask for administrator rights; portable installs are per user and can be rolled back.") {
                    d.install_mode_chosen = true;
                }
            }
            Os::Linux => {
                if let Some(format) = oktospace_core::platform::linux_package_format() {
                    let (ext, tool) = if format == oktospace_core::platform::AssetKind::Rpm {
                        (".rpm", "dnf")
                    } else {
                        (".deb", "apt")
                    };
                    divider(ui, p);
                    if switch_row(
                        ui,
                        p,
                        free("prefer_system_installer"),
                        &mut d.prefer_system_installer,
                        &format!("Install as system packages ({ext})"),
                        &format!(
                            "Through {tool}: the package manager owns the files and installing asks for your password."
                        ),
                    ) {
                        d.install_mode_chosen = true;
                    }
                }
                divider(ui, p);
                switch_row(
                    ui,
                    p,
                    free("prefer_appimage"),
                    &mut d.prefer_appimage,
                    "Prefer AppImages",
                    "Updates then download only what changed (from the release's .zsync file).",
                );
            }
            _ => {}
        }
    });
}

fn appearance(app: &mut OktoSpaceApp, ui: &mut Ui, p: &theme::Palette, d: &mut Settings) {
    group(ui, p, |ui| {
        row(ui, p, "Theme", "", |ui| {
            ui.selectable_value(&mut d.theme, Theme::System, "Match system");
            ui.selectable_value(&mut d.theme, Theme::Light, "Light");
            ui.selectable_value(&mut d.theme, Theme::Dark, "Dark");
        });
        if app.manager.platform().os == Os::Macos {
            use oktospace_core::settings::DockIcon;
            divider(ui, p);
            ui.label(RichText::new("Dock icon").size(14.0));
            ui.radio_value(
                &mut d.dock_icon,
                DockIcon::ColorWhenOpen,
                "Full colour while the window is open, macOS's style otherwise",
            );
            ui.radio_value(&mut d.dock_icon, DockIcon::Color, "Always full colour");
            ui.radio_value(&mut d.dock_icon, DockIcon::System, "Always macOS's icon style (dark, clear or tinted)");
            ui.label(
                RichText::new("The icon style is chosen in System Settings › Appearance.").size(12.0).color(p.weak),
            );
        }
    });
}

fn github_apps(app: &mut OktoSpaceApp, ui: &mut Ui, p: &theme::Palette, policy: &Policy, d: &mut Settings) {
    group(ui, p, |ui| {
        switch_row(
            ui,
            p,
            !policy.locks("other_sources"),
            &mut d.other_sources.enabled,
            "Apps from any GitHub repository",
            "Add apps besides the App library, or update an app (or OktoSpace) from a fork, a mirror or a backup.",
        );
    });
    if !(app.settings.other_sources.enabled && d.other_sources.enabled) {
        if d.other_sources.enabled {
            ui.label(RichText::new("Save to turn them on, then add apps here.").size(12.0).color(p.warn));
        }
        return;
    }
    let other = app.settings.other_sources.clone();
    group_title(ui, p, "Added apps and sources");
    group(ui, p, |ui| {
        for custom in &other.apps {
            let installed = app.manager.installed_app(&custom.id).is_some();
            row(ui, p, &custom.name, &format!("github.com/{}", custom.repo), |ui| {
                if ui
                    .add_enabled(!installed, egui::Button::new("Remove").small())
                    .on_disabled_hover_text("Uninstall it first")
                    .clicked()
                {
                    app.actions.push(Action::Source(crate::worker::SourceOp::Remove { id: custom.id.clone() }));
                }
            });
            divider(ui, p);
        }
        for (id, repo) in &other.overrides {
            row(ui, p, &app.app_name(id), &format!("Updates from github.com/{repo}"), |ui| {
                if ui.add(egui::Button::new("Change…").small()).clicked() {
                    app.actions.push(Action::OpenSource(crate::views::sources::Target::App(id.clone())));
                }
            });
            divider(ui, p);
        }
        row(
            ui,
            p,
            "OktoSpace",
            &format!("Updates from github.com/{}", oktospace_core::selfupdate::update_repo(&app.manager)),
            |ui| {
                if ui.add(egui::Button::new("Change…").small()).clicked() {
                    app.actions.push(Action::OpenSource(crate::views::sources::Target::App(
                        oktospace_core::selfupdate::NAME.into(),
                    )));
                }
            },
        );
    });
    ui.horizontal(|ui| {
        if ui.button("Add an app from GitHub…").clicked() {
            app.actions.push(Action::OpenSource(crate::views::sources::Target::NewApp));
        }
        if ui.button("Find on GitHub…").clicked() {
            app.actions.push(Action::OpenGithub);
        }
    });
    ui.label(
        RichText::new("To update a library app from a fork, use \"Update source…\" in its ••• menu.")
            .size(12.0)
            .color(p.weak),
    );
}

fn build_tools(app: &mut OktoSpaceApp, ui: &mut Ui, p: &theme::Palette) {
    let Some(tools) = app.toolchains.clone() else { return };
    group(ui, p, |ui| {
        for (i, (tool, found)) in tools.iter().enumerate() {
            if i > 0 {
                divider(ui, p);
            }
            match found {
                Some(path) => row(ui, p, tool.name, &path.display().to_string(), |ui| {
                    ui.label(RichText::new("Installed").size(12.5).color(p.good));
                }),
                None => row(ui, p, tool.name, "Not installed", |ui| {
                    if ui.add(egui::Button::new(RichText::new("Get it ↗").color(p.accent)).frame(false)).clicked() {
                        app.actions.push(Action::OpenUrl(tool.url.to_string()));
                    }
                }),
            }
        }
    });
    ui.label(RichText::new("oktospace-cli doctor shows their versions too.").size(12.0).color(p.weak));
}

fn github(ui: &mut Ui, p: &theme::Palette, policy: &Policy, d: &mut Settings) {
    group(ui, p, |ui| {
        ui.label(RichText::new("Access token").size(14.0));
        ui.add(egui::Label::new(RichText::new("Without one, GitHub allows 60 requests an hour; OktoSpace caches and falls back to direct downloads when that runs out. A token with no scopes raises it to 5,000.").size(12.0).color(p.weak)).wrap());
        let mut token = d.github_token.clone().unwrap_or_default();
        ui.add_enabled(
            !policy.locks("github_token"),
            egui::TextEdit::singleline(&mut token)
                .password(true)
                .hint_text("Personal access token (optional)")
                .desired_width(f32::INFINITY),
        );
        d.github_token = (!token.trim().is_empty()).then(|| token.trim().to_string());
        divider(ui, p);
        switch_row(
            ui,
            p,
            !policy.locks("remote_catalog"),
            &mut d.remote_catalog,
            "Get new apps from the online catalog",
            "The App library grows without updating OktoSpace.",
        );
    });
}

fn your_apps(app: &mut OktoSpaceApp, ui: &mut Ui, p: &theme::Palette) {
    group(ui, p, |ui| {
        row(
            ui,
            p,
            "App list",
            "The installed apps with their versions and channels, to set up another computer the same way.",
            |ui| {
                if ui.button("Import…").clicked() {
                    app.actions.push(Action::ImportList);
                }
                if ui.button("Export…").clicked() {
                    app.actions.push(Action::ExportList);
                }
            },
        );
    });
}

fn about(app: &mut OktoSpaceApp, ui: &mut Ui, p: &theme::Palette, policy: &Policy) {
    group(ui, p, |ui| {
        row(
            ui,
            p,
            &format!("OktoSpace {}", crate::app::version_string()),
            &format!(
                "On {}. An open-source installer, updater and builder for apps on GitHub.",
                app.manager.platform().display()
            ),
            |ui| {
                if ui.button("Source code ↗").clicked() {
                    app.actions.push(Action::OpenUrl("https://github.com/emircesur/OktoSpace".into()));
                }
            },
        );
        divider(ui, p);
        row(ui, p, "Data folder", &app.manager.paths().root.display().to_string(), |ui| {
            if ui.button("Open").clicked() {
                app.actions.push(Action::OpenPath(app.manager.paths().root.clone()));
            }
        });
        if let Some(src) = &policy.source {
            divider(ui, p);
            row(ui, p, "Policy", &src.display().to_string(), |_| {});
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Keyboard: Ctrl+K opens the command palette").clicked() {
            app.actions.push(Action::OpenPalette);
        }
    });
}

/// What the organization's policy sets, and the tools for computer labs: a shared package
/// cache, status reports.
fn it_section(app: &mut OktoSpaceApp, ui: &mut Ui, p: &theme::Palette, policy: &Policy, draft: &mut Settings) {
    let weak = |t: String| RichText::new(t).size(12.0).color(p.weak);
    match &policy.source {
        Some(src) => {
            let org = policy.organization.as_deref().unwrap_or("An organization");
            ui.label(format!("{org} manages OktoSpace on this computer."));
            ui.label(weak(format!("Policy file: {}", src.display())));
            if let Some(url) = &policy.policy_url {
                ui.label(weak(format!("Updated from: {url}")));
            }
            let mut facts = Vec::new();
            if let Some(w) = &policy.update_window {
                facts.push(format!("Updates install {}.", w.describe()));
            }
            if !policy.required_apps.is_empty() {
                facts.push(format!("Always installed: {}.", names(app, &policy.required_apps)));
            }
            if !policy.pinned_versions.is_empty() {
                let pins: Vec<String> =
                    policy.pinned_versions.iter().map(|(id, v)| format!("{} {v}", app.app_name(id))).collect();
                facts.push(format!("Kept on: {}.", pins.join(", ")));
            }
            if !policy.blocked_apps.is_empty() {
                facts.push(format!("Not available: {}.", names(app, &policy.blocked_apps)));
            }
            if policy.prevent_uninstall {
                facts.push("Apps can't be uninstalled or rolled back (except by an administrator).".into());
            }
            if let Some(dir) = &policy.report_dir {
                facts.push(format!("A status report is saved to {} after each check.", dir.display()));
            }
            for f in facts {
                ui.label(f);
            }
            if let Some(support) = &policy.support {
                ui.label(format!("Help: {support}"));
            }
        }
        None => {
            ui.label(weak(
                "Not managed. For labs and classrooms, a policy file can install apps on every computer, keep them on a version, \
                 limit updates to after-school hours and lock settings: run `oktospace-cli policy show` for where it goes, \
                 and see \"Managing many computers\" in the README."
                    .into(),
            ));
        }
    }
    ui.add_space(4.0);
    ui.label("Package cache (a shared folder, so a room of computers downloads each app once)");
    let locked = policy.locks("package_cache");
    ui.horizontal(|ui| {
        let text = draft.package_cache.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| "Not used".into());
        ui.add(egui::Label::new(RichText::new(text).monospace()).truncate());
        ui.add_enabled_ui(!locked, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if draft.package_cache.is_some() && ui.button("Don't use").clicked() {
                    draft.package_cache = None;
                }
                if ui.button("Choose…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().set_title("Package cache folder").pick_folder() {
                        draft.package_cache = Some(dir);
                    }
                }
            });
        });
    });
    if draft.package_cache.is_some() {
        ui.add_enabled(
            !policy.locks("package_cache_write"),
            egui::Checkbox::new(&mut draft.package_cache_write, "Also put what this computer downloads into the cache"),
        );
        ui.label(weak("Packages are checked against their published checksums before they're used.".into()));
    }
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Save a status report…")
            .on_hover_text("This computer, OktoSpace and every app's version, as JSON")
            .clicked()
        {
            app.actions.push(Action::SaveReport);
        }
        let cache_set = app.settings.package_cache.is_some();
        if ui
            .add_enabled(cache_set, egui::Button::new("Fill the package cache now"))
            .on_hover_text("Download the current version of the installed apps into the cache")
            .on_disabled_hover_text("Choose a package cache folder and save first")
            .clicked()
        {
            app.actions.push(Action::FillCache);
        }
    });
}

fn names(app: &OktoSpaceApp, ids: &[String]) -> String {
    ids.iter().map(|id| app.app_name(id)).collect::<Vec<_>>().join(", ")
}

/// Disk space used by each app (and its earlier version), downloads, builds and the cache.
fn storage_section(app: &mut OktoSpaceApp, ui: &mut Ui, p: &theme::Palette) {
    use oktospace_core::download::format_bytes;
    let Some(storage) = app.storage.clone() else { return };
    let catalog = app.manager.catalog();
    let kept: u64 = storage.apps.iter().map(|a| a.previous).sum();
    let total = storage.apps.iter().map(|a| a.current + a.previous).sum::<u64>()
        + storage.downloads
        + storage.builds
        + storage.cache;
    ui.label(format!("OktoSpace uses {} in all.", format_bytes(total)));
    egui::Grid::new("storage").num_columns(3).spacing([16.0, 4.0]).striped(true).show(ui, |ui| {
        for a in &storage.apps {
            let name =
                catalog.apps.iter().find(|e| e.id == a.id).map(|e| e.name.clone()).unwrap_or_else(|| a.id.clone());
            ui.label(name);
            ui.label(format_bytes(a.current));
            if a.previous > 0 {
                if ui
                    .small_button(format!("Remove earlier version ({})", format_bytes(a.previous)))
                    .on_hover_text("Frees the space, but the app can't be rolled back any more")
                    .clicked()
                {
                    app.actions.push(Action::TrimPrevious(a.id.clone()));
                }
            } else {
                ui.label("");
            }
            ui.end_row();
        }
        for (label, bytes) in [
            ("Downloads", storage.downloads),
            ("Build folders", storage.builds),
            ("Cache (release information, icons)", storage.cache),
        ] {
            ui.label(RichText::new(label).color(p.weak));
            ui.label(format_bytes(bytes));
            ui.label("");
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        if storage.downloads + storage.builds > 0 && ui.button("Clear downloads and build files").clicked() {
            app.actions.push(Action::Cleanup);
            app.storage = None;
        }
        if kept > 0 && ui.button(format!("Remove all earlier versions ({})", format_bytes(kept))).clicked() {
            for a in storage.apps.iter().filter(|a| a.previous > 0) {
                app.actions.push(Action::TrimPrevious(a.id.clone()));
            }
        }
    });
}
