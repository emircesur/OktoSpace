//! The command palette (Ctrl/⌘+K): type to jump to an app, install, update or open it, or run any
//! of OktoSpace's commands, without leaving the keyboard.

use eframe::egui::{self, RichText};

use crate::app::{Action, AppsView, OktoSpaceApp};

#[derive(Default)]
pub struct CommandPalette {
    pub query: String,
    pub selected: usize,
}

struct Item {
    label: String,
    hint: String,
    action: Action,
    score: i32,
}

/// How well `text` matches `query`: higher is better, `None` for no match. Whole-word and prefix
/// matches beat matches inside a word, which beat letters found in order ("ffx" → "Firefox").
fn score(text: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let (t, q) = (text.to_lowercase(), query.to_lowercase());
    if let Some(i) = t.find(&q) {
        let word_start = i == 0 || !t.as_bytes()[i - 1].is_ascii_alphanumeric();
        return Some(1000 - i as i32 + if word_start { 500 } else { 0 } - t.len() as i32);
    }
    // Letters in order, close together ("vsc" → "VS Code", but not "okto" → "Tonkatsu Box").
    let t: Vec<char> = t.chars().collect();
    let (mut at, mut first) = (0, None);
    for c in q.chars().filter(|c| !c.is_whitespace()) {
        let i = at + t[at..].iter().position(|&x| x == c)?;
        first.get_or_insert(i);
        at = i + 1;
    }
    let first = first.unwrap_or(0);
    let word_start = first == 0 || !t[first - 1].is_alphanumeric();
    (word_start && at - first <= 2 * q.chars().count() + 1).then(|| 100 - t.len() as i32)
}

/// Like [`score`], but only for the text containing the query (for descriptions).
fn contains_score(text: &str, query: &str) -> Option<i32> {
    (!query.is_empty() && text.to_lowercase().contains(&query.to_lowercase())).then(|| score(text, query)).flatten()
}

fn items(app: &OktoSpaceApp, query: &str) -> Vec<Item> {
    let q = query.trim();
    let mut out = Vec::new();
    let mut command = |label: &str, hint: &str, action: Action| {
        if let Some(score) = score(label, q) {
            out.push(Item { label: label.into(), hint: hint.into(), action, score: score + 50 });
        }
    };
    let updates = app.update_count();
    command("Check for updates", "Ctrl+R", Action::Refresh);
    if updates > 0 {
        command(&format!("Update all ({updates})"), "", Action::UpdateAll);
    }
    command("Go to App library", "", Action::GoApps(AppsView::All));
    command("Go to Updates", "", Action::GoApps(AppsView::Updates));
    command("Go to Installed", "", Action::GoApps(AppsView::Installed));
    command("Go to Favorites", "", Action::GoApps(AppsView::Favorites));
    command("Go to Activity", "what OktoSpace did", Action::GoApps(AppsView::Activity));
    command("Find on GitHub…", "search all of GitHub, or someone's stars", Action::OpenGithub);
    command("Install several apps…", "", Action::OpenMultiInstall);
    command("Settings", "", Action::OpenSettings);
    command("Export app list…", "", Action::ExportList);
    command("Import app list…", "", Action::ImportList);
    command("Clear downloads and build files", "", Action::Cleanup);
    command("About OktoSpace", "", Action::OpenAbout);

    // `owner/repo`: add it straight away.
    let repo = q.trim_start_matches("https://github.com/").trim_end_matches('/');
    if repo.split('/').count() == 2 && repo.split('/').all(|p| !p.is_empty() && !p.contains(' ')) {
        out.push(Item {
            label: format!("Add {repo} from GitHub"),
            hint: "install it from its releases, or build it".into(),
            action: Action::AddRepo(repo.to_string()),
            score: 5000,
        });
    }

    if !q.is_empty() {
        for s in &app.states {
            let a = &s.app;
            let Some(best) = [
                score(&a.name, q).map(|x| x + 200),
                contains_score(&a.repo, q),
                contains_score(&a.tagline, q).map(|x| x - 600),
            ]
            .into_iter()
            .flatten()
            .max() else {
                continue;
            };
            let mut app_item = |verb: &str, action: Action, bonus: i32| {
                out.push(Item {
                    label: format!("{verb} {}", a.name),
                    hint: a.tagline.clone(),
                    action,
                    score: best + bonus,
                });
            };
            if s.installed.is_some() {
                app_item("Open", Action::Launch(a.id.clone(), Vec::new()), 30);
            }
            if s.update_available {
                app_item("Update", Action::Update(a.id.clone()), 25);
            } else if s.installed.is_none() && s.installable.is_some() {
                app_item("Install", Action::Install(a.id.clone(), None), 20);
            }
            app_item("Show", Action::ShowApp(a.id.clone()), 10);
        }
    }
    out.sort_by_key(|i| std::cmp::Reverse(i.score));
    out.truncate(40);
    out
}

pub fn window(app: &mut OktoSpaceApp, ctx: &egui::Context) {
    let Some(mut pal) = app.palette_open.take() else { return };
    let p = app.palette;
    let list = items(app, &pal.query);
    pal.selected = pal.selected.min(list.len().saturating_sub(1));
    let (down, up, enter) = ctx.input(|i| {
        (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::Enter))
    });
    if down {
        pal.selected = (pal.selected + 1).min(list.len().saturating_sub(1));
    }
    if up {
        pal.selected = pal.selected.saturating_sub(1);
    }
    let mut chosen: Option<Action> = None;
    let modal = egui::Modal::new(egui::Id::new("command-palette")).show(ctx, |ui| {
        ui.set_width(560.0);
        let edit = ui.add(
            egui::TextEdit::singleline(&mut pal.query)
                .hint_text("Type an app or a command…  (owner/repo adds a repository)")
                .desired_width(f32::INFINITY)
                .font(egui::FontId::proportional(16.0)),
        );
        edit.request_focus();
        if edit.changed() {
            pal.selected = 0;
        }
        ui.add_space(6.0);
        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (i, item) in list.iter().enumerate() {
                let selected = i == pal.selected;
                let frame = egui::Frame::new()
                    .fill(if selected { p.accent.gamma_multiply(0.16) } else { egui::Color32::TRANSPARENT })
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(10, 4));
                let response = frame
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&item.label).color(if selected { p.accent } else { p.text }));
                            if !item.hint.is_empty() {
                                ui.add(egui::Label::new(RichText::new(&item.hint).size(12.0).color(p.weak)).truncate());
                            }
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                if selected && (down || up) {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    chosen = Some(item.action.clone());
                }
            }
            if list.is_empty() {
                ui.label(RichText::new("Nothing matches.").color(p.weak));
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Up/Down to choose · Enter to run · Esc to close").size(11.5).color(p.weak));
    });
    if enter {
        chosen = chosen.or_else(|| list.get(pal.selected).map(|i| i.action.clone()));
    }
    if let Some(action) = chosen {
        app.actions.push(action);
        return;
    }
    if !modal.should_close() {
        app.palette_open = Some(pal);
    }
}

#[cfg(test)]
mod tests {
    use super::score;

    #[test]
    fn scores_prefixes_over_inner_and_scattered_matches() {
        let prefix = score("Firefox", "fire").unwrap();
        let inner = score("Campfire", "fire").unwrap();
        let scattered = score("Firefox", "ffx").unwrap();
        assert!(prefix > inner && inner > scattered);
        assert_eq!(score("Zed", "xyz"), None);
        assert!(score("VS Code", "vsc").is_some());
        assert_eq!(score("Tonkatsu Box", "okto"), None);
        assert_eq!(score("anything", ""), Some(0));
    }
}
