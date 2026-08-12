#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod custom;
mod data;
mod model;
mod settings;
mod theme;

#[cfg(target_os = "windows")]
#[path = "about_v2.rs"]
mod about;
#[cfg(target_os = "windows")]
mod app;
#[cfg(target_os = "windows")]
mod caret;
#[cfg(target_os = "windows")]
#[path = "manager_v2.rs"]
mod manager;
#[cfg(target_os = "windows")]
mod native_window;
#[cfg(target_os = "windows")]
mod preview;
#[cfg(target_os = "windows")]
mod renderer;
#[cfg(target_os = "windows")]
mod update;

#[cfg(target_os = "windows")]
fn main() {
    unsafe {
        // The preview integration observes picker messages after the main wndproc
        // has updated selection/hover state. It never activates its own window.
        preview::install_thread_hook();
    }

    let result = app::run();

    unsafe {
        preview::shutdown();
        preview::uninstall_thread_hook();
    }

    if let Err(error) = result {
        app::show_error(&error);
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("windows-emojipicker is a Windows-only application.");
}
