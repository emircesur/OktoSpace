//! The engine behind OktoSpace, the installer, updater and builder for apps on GitHub.
//!
//! Start with [`manager::Manager`]: it ties together the app [`catalog`], release discovery on
//! [`github`], [`download`]s with checksum verification, unpacking ([`archive`]), desktop
//! [`integrate`]ion and the record of what is installed ([`state`]).

pub mod archive;
pub mod build;
pub mod catalog;
pub mod detect;
pub mod download;
pub mod github;
pub mod http;
pub mod integrate;
pub mod manager;
pub mod paths;
pub mod platform;
pub mod selfupdate;
pub mod settings;
pub mod sources;
pub mod state;
pub mod version;

pub use catalog::{AppEntry, Catalog};
pub use download::{Progress, ProgressEvent, Stage};
pub use manager::{AppState, Manager, Plan};
pub use settings::Settings;
pub mod autostart;
pub mod icons;
pub mod policy;
pub mod readme;
pub mod running;
pub mod tour;
pub mod zsync;
