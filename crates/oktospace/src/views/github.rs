//! Find on GitHub: search every repository on GitHub, or list the repositories someone starred,
//! and add any of them to the App library (installed from its releases, or built from source).

use std::collections::HashSet;

use eframe::egui::{self, Align, Layout, RichText};
use oktospace_core::github::RepoHit;

use crate::app::{Action, OktoSpaceApp};
use crate::theme;

#[derive(Default)]
pub struct Dialog {
    /// Search GitHub (false) or a user's stars (true).
    pub stars: bool,
    pub query: String,
    pub busy: bool,
    pub error: Option<String>,
    pub results: Vec<RepoHit>,
    /// Apps added from this window (by id).
    pub added: HashSet<String>,
}

pub fn window(app: &mut OktoSpaceApp, ctx: &egui::Context) {
    let Some(mut d) = app.github.take() else { return };
    let p = app.palette;
    let mut open = true;
    let catalog = app.manager.catalog();
    egui::Window::new("Find on GitHub")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([640.0, 560.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(!d.stars, "Search GitHub").clicked() && d.stars {
                    d.stars = false;
                    d.results.clear();
                }
                if ui.selectable_label(d.stars, "Someone's stars").clicked() && !d.stars {
                    d.stars = true;
                    d.results.clear();
                }
            });
            ui.label(
                RichText::new(if d.stars {
                    "A GitHub user's starred repositories: add the apps among them in one go."
                } else {
                    "Any repository on GitHub, most starred first. OktoSpace installs it from its releases, or builds it from source when it has none."
                })
                .size(12.5)
                .color(p.weak),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let hint = if d.stars { "GitHub user name" } else { "Search: markdown editor, terminal, ripgrep…" };
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut d.query).hint_text(hint).desired_width(ui.available_width() - 90.0),
                );
                let go = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (theme::primary(ui, &p, if d.stars { "Show" } else { "Search" }).clicked() || go)
                    && !d.query.trim().is_empty()
                    && !d.busy
                {
                    let q = d.query.trim().to_string();
                    app.actions.push(if d.stars { Action::LoadStars(q) } else { Action::SearchGithub(q) });
                }
            });
            ui.add_space(6.0);
            if d.busy {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label("Asking GitHub…");
                });
            }
            if let Some(err) = &d.error {
                ui.label(RichText::new(err).color(p.bad));
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for hit in d.results.iter().filter(|h| !h.archived) {
                    let in_library = catalog.apps.iter().find(|a| a.repo.eq_ignore_ascii_case(&hit.repo));
                    theme::card_frame(&p, false).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width() - 110.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&hit.repo).strong());
                                    ui.label(
                                        RichText::new(format!("★ {}", super::apps::short_count(hit.stars)))
                                            .size(12.0)
                                            .color(p.weak),
                                    );
                                    if let Some(lang) = &hit.language {
                                        ui.label(RichText::new(lang).size(12.0).color(p.weak));
                                    }
                                });
                                if !hit.description.is_empty() {
                                    ui.add(egui::Label::new(RichText::new(&hit.description).size(12.5).color(p.weak)).wrap());
                                }
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| match in_library {
                                Some(a) => {
                                    if theme::secondary(ui, &p, "Open").clicked() {
                                        app.actions.push(Action::ShowApp(a.id.clone()));
                                    }
                                }
                                None => {
                                    if theme::primary(ui, &p, "Add").clicked() {
                                        app.actions.push(Action::AddRepo(hit.repo.clone()));
                                    }
                                }
                            });
                        });
                    });
                    ui.add_space(4.0);
                }
                if !d.busy && d.results.is_empty() && d.error.is_none() && !d.query.is_empty() {
                    ui.label(RichText::new("Nothing yet: press Enter to search.").color(p.weak));
                }
            });
        });
    if open {
        app.github = Some(d);
    }
}
