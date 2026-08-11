use std::{fs, path::PathBuf, sync::{Mutex, OnceLock}};

use serde::{Deserialize, Serialize};

use crate::model::ItemKind;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomItem {
    pub kind: ItemKind,
    pub title: String,
    pub content: String,
    pub keywords: String,
}

static ITEMS: OnceLock<Mutex<Vec<CustomItem>>> = OnceLock::new();

fn cell() -> &'static Mutex<Vec<CustomItem>> {
    ITEMS.get_or_init(|| Mutex::new(load_from_disk()))
}

pub fn snapshot() -> Vec<CustomItem> {
    cell().lock().unwrap().clone()
}

pub fn with_items<R>(callback: impl FnOnce(&[CustomItem]) -> R) -> R {
    let items = cell().lock().unwrap();
    callback(&items)
}

pub fn get(index: usize) -> Option<CustomItem> {
    cell().lock().unwrap().get(index).cloned()
}

pub fn upsert(index: Option<usize>, mut item: CustomItem) -> usize {
    if item.title.trim().is_empty() {
        item.title = item.content.lines().next().unwrap_or("Custom").chars().take(32).collect();
        if item.title.trim().is_empty() { item.title = "Custom".to_string(); }
    }

    let mut items = cell().lock().unwrap();
    let selected = if let Some(index) = index.filter(|index| *index < items.len()) {
        items[index] = item;
        index
    } else {
        items.push(item);
        items.len() - 1
    };
    save_to_disk(&items);
    selected
}

pub fn remove(index: usize) -> bool {
    let mut items = cell().lock().unwrap();
    if index >= items.len() { return false; }
    items.remove(index);
    save_to_disk(&items);
    true
}

fn load_from_disk() -> Vec<CustomItem> {
    let Ok(text) = fs::read_to_string(custom_path()) else { return Vec::new(); };
    serde_json::from_str::<Vec<CustomItem>>(&text)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| matches!(item.kind, ItemKind::Kaomoji | ItemKind::Ascii | ItemKind::Snippet))
        .collect()
}

fn save_to_disk(items: &[CustomItem]) {
    let path = custom_path();
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    if let Ok(text) = serde_json::to_string_pretty(items) {
        let _ = fs::write(path, text);
    }
}

fn custom_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("WindowsEmojiPicker").join("custom-items.json")
}
