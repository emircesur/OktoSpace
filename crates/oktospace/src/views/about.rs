//! About OktoSpace: the logo, version, what it is and where it comes from.

use eframe::egui::{self, Align, Layout, RichText, Vec2};

use crate::app::{Action, OktoSpaceApp};
use crate::theme;

const REPO: &str = "https://github.com/emircesur/OktoSpace";

pub fn window(app: &mut OktoSpaceApp, ctx: &egui::Context) {
    if !app.about_open {
        return;
    }
    let p = app.palette;
    let mut keep = true;
    let modal = egui::Modal::new(egui::Id::new("about")).show(ctx, |ui| {
        ui.set_width(460.0);
        ui.vertical_centered(|ui| {
            ui.add_space(6.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(128.0), egui::Sense::hover());
            egui::Image::new(&theme::logo_large(ctx)).corner_radius(24).paint_at(ui, rect);
            ui.add_space(10.0);
            ui.label(RichText::new("OktoSpace").size(28.0).strong());
            ui.label(RichText::new(format!("Version {}", crate::app::version_string())).color(p.weak));
            ui.label(RichText::new(app.manager.platform().display()).size(12.0).color(p.weak));
            ui.add_space(10.0);
            ui.add(
                egui::Label::new(
                    "An installer, updater and builder for apps on GitHub: install from releases or build from source, keep everything up to date and roll back, all in one place.",
                )
                .wrap(),
            );
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Free and open source (MIT or Apache-2.0). Not affiliated with GitHub or with the apps' developers. App names and icons belong to their owners.",
                    )
                    .size(12.0)
                    .color(p.weak),
                )
                .wrap(),
            );
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "No accounts, no tracking: OktoSpace only talks to GitHub, to find releases, READMEs and icons and to get source code. Your files are never uploaded.",
                    )
                    .size(12.0)
                    .color(p.weak),
                )
                .wrap(),
            );
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                for (label, url) in [
                    ("Source code ↗", REPO.to_string()),
                    ("Releases ↗", format!("{REPO}/releases")),
                    ("Report a problem ↗", format!("{REPO}/issues/new")),
                ] {
                    if ui.add(egui::Button::new(RichText::new(label).color(p.accent)).frame(false)).clicked() {
                        app.actions.push(Action::OpenUrl(url));
                    }
                }
            });
        });
        ui.add_space(10.0);
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::pill(ui, &p, "Close").clicked() {
                keep = false;
            }
        });
    });
    if modal.should_close() {
        keep = false;
    }
    app.about_open = keep;
}
