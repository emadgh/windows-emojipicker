#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod custom;
mod data;
mod model;
mod settings;
mod theme;

#[cfg(target_os = "windows")]
mod about;
#[cfg(target_os = "windows")]
mod app;
#[cfg(target_os = "windows")]
mod caret;
#[cfg(target_os = "windows")]
mod manager;
#[cfg(target_os = "windows")]
mod renderer;
#[cfg(target_os = "windows")]
mod update;

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = app::run() {
        app::show_error(&error);
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("windows-emojipicker is a Windows-only application.");
}
