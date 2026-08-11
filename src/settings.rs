use std::{fs, path::PathBuf, sync::{Mutex, OnceLock}};

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
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: Theme::Dark, auto_update: true }
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

fn load_from_disk() -> Settings {
    let mut settings = Settings::default();
    if let Ok(text) = fs::read_to_string(settings_path()) {
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue; };
            match key.trim() {
                "theme" => settings.theme = Theme::from_key(value.trim()),
                "auto_update" => settings.auto_update = matches!(value.trim(), "true" | "1" | "yes" | "on"),
                _ => {}
            }
        }
    }
    settings
}

fn save_to_disk(settings: Settings) {
    let path = settings_path();
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    let text = format!("theme={}\nauto_update={}\n", settings.theme.key(), settings.auto_update);
    let _ = fs::write(path, text);
}

fn settings_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("WindowsEmojiPicker").join("settings.ini")
}
