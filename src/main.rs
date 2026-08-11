#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod data;
mod model;

#[cfg(target_os = "windows")]
mod app;

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
