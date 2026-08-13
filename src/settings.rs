use std::{
    fs,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::Command,
    sync::{Mutex, OnceLock},
};

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "WindowsEmojiPicker";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn key(self) -> &'static str {
        match self { Self::Dark => "dark", Self::Light => "light" }
    }

    pub fn from_key(value: &str) -> Self {
        if value.eq_ignore_ascii_case("light") { Self::Light } else { Self::Dark }
    }

    pub fn toggle(self) -> Self {
        match self { Self::Dark => Self::Light, Self::Light => Self::Dark }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub theme: Theme,
    pub auto_update: bool,
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            auto_update: true,
            // New installs start with Windows by default. Existing installs are
            // migrated once because their settings.ini has no autostart key.
            autostart: true,
        }
    }
}

static SETTINGS: OnceLock<Mutex<Settings>> = OnceLock::new();

fn cell() -> &'static Mutex<Settings> {
    SETTINGS.get_or_init(|| Mutex::new(load_from_disk()))
}

pub fn get() -> Settings {
    *cell().lock().unwrap()
}

pub fn toggle_theme() -> Theme {
    let mut settings = cell().lock().unwrap();
    settings.theme = settings.theme.toggle();
    save_to_disk(*settings);
    settings.theme
}

pub fn set_auto_update(value: bool) {
    let mut settings = cell().lock().unwrap();
    settings.auto_update = value;
    save_to_disk(*settings);
}

/// Enable or disable launch-at-login using the same HKCU Run-key strategy as
/// GahYar. The registry is the source of truth; settings.ini mirrors it for
/// persistence and UI display.
pub fn set_autostart(value: bool) -> bool {
    if !set_autostart_registry(value) {
        return false;
    }

    let mut settings = cell().lock().unwrap();
    settings.autostart = value;
    save_to_disk(*settings);
    true
}

fn load_from_disk() -> Settings {
    let mut settings = Settings::default();
    let mut saw_autostart_key = false;

    if let Ok(text) = fs::read_to_string(settings_path()) {
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue; };
            match key.trim() {
                "theme" => settings.theme = Theme::from_key(value.trim()),
                "auto_update" => settings.auto_update = parse_bool(value),
                "autostart" => {
                    settings.autostart = parse_bool(value);
                    saw_autostart_key = true;
                }
                _ => {}
            }
        }
    }

    // v0.3.7 and earlier do not have an autostart setting. Migrate those
    // installs once by enabling the Run entry. After the key exists in the
    // settings file, the registry remains authoritative and manual registry
    // edits are respected instead of being overwritten on every launch.
    if !saw_autostart_key && settings.autostart {
        let _ = set_autostart_registry(true);
    }
    settings.autostart = is_autostart_enabled();

    // Persist the migrated key immediately so a deliberate user disable is not
    // undone on a later launch, even if the application exits before any other
    // setting changes.
    if !saw_autostart_key {
        save_to_disk(settings);
    }

    settings
}

fn save_to_disk(settings: Settings) {
    let path = settings_path();
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    let text = format!(
        "theme={}\nauto_update={}\nautostart={}\n",
        settings.theme.key(),
        settings.auto_update,
        settings.autostart,
    );
    let _ = fs::write(path, text);
}

fn set_autostart_registry(enabled: bool) -> bool {
    if !enabled && !is_autostart_enabled() {
        return true;
    }

    let mut command = Command::new("reg.exe");
    command.creation_flags(CREATE_NO_WINDOW);

    if enabled {
        let Ok(executable) = std::env::current_exe() else { return false; };
        let value = format!("\"{}\"", executable.display());
        command.args([
            "add",
            RUN_KEY,
            "/v",
            RUN_VALUE,
            "/t",
            "REG_SZ",
            "/d",
            &value,
            "/f",
        ]);
    } else {
        command.args(["delete", RUN_KEY, "/v", RUN_VALUE, "/f"]);
    }

    command.status().map(|status| status.success()).unwrap_or(false)
}

fn is_autostart_enabled() -> bool {
    Command::new("reg.exe")
        .args(["query", RUN_KEY, "/v", RUN_VALUE])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn parse_bool(value: &str) -> bool {
    matches!(value.trim(), "true" | "1" | "yes" | "on")
}

fn settings_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("WindowsEmojiPicker").join("settings.ini")
}
