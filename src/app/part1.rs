use std::{
    cell::RefCell,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    thread,
    time::Duration,
};

use windows_sys::{
    core::PCWSTR,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{
            Input::KeyboardAndMouse::*,
            Shell::*,
            WindowsAndMessaging::*,
        },
    },
};

use crate::{
    data::ITEMS,
    model::{ItemKind, PickerItem},
};

const APP_NAME: &str = "Windows Emoji Picker";
const WINDOW_CLASS: &str = "WindowsEmojiPicker.NativeWindow";

const HOTKEY_ID: i32 = 1;
const TRAY_ICON_ID: u32 = 1;
const WM_TRAY: u32 = WM_APP + 1;
const CMD_OPEN: usize = 1001;
const CMD_EXIT: usize = 1002;

const POPUP_W: i32 = 620;
const POPUP_H: i32 = 440;
const PAD: i32 = 12;
const SEARCH_Y: i32 = 12;
const SEARCH_H: i32 = 44;
const TABS_Y: i32 = 64;
const TABS_H: i32 = 32;
const CONTENT_TOP: i32 = 108;
const FOOTER_Y: i32 = 408;
const FOOTER_H: i32 = 24;
const COLS: usize = 4;
const VISIBLE_ROWS: usize = 4;
const GAP: i32 = 8;
const CARD_H: i32 = 64;

thread_local! {
    static STATE: RefCell<AppState> = RefCell::new(AppState::default());
}

struct AppState {
    hwnd: HWND,
    target_hwnd: HWND,
    category_index: usize,
    query_utf16: Vec<u16>,
    selected: usize,
    scroll: usize,
    hovered: Option<usize>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            hwnd: null_mut(),
            target_hwnd: null_mut(),
            category_index: 0,
            query_utf16: Vec::new(),
            selected: 0,
            scroll: 0,
            hovered: None,
        }
    }
}

pub fn run() -> Result<(), String> {
    unsafe {
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err("GetModuleHandleW failed".into());
        }

        let class_name = wide(WINDOW_CLASS);
        let window_title = wide(APP_NAME);
        let icon = LoadIconW(null_mut(), IDI_APPLICATION);
        let cursor = LoadCursorW(null_mut(), IDC_ARROW);

        let mut wc: WNDCLASSW = zeroed();
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.lpfnWndProc = Some(wnd_proc);
        wc.hInstance = instance;
        wc.hIcon = icon;
        wc.hCursor = cursor;
        wc.hbrBackground = null_mut();
        wc.lpszClassName = class_name.as_ptr();

        if RegisterClassW(&wc) == 0 {
            return Err("RegisterClassW failed".into());
        }

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_POPUP,
            0,
            0,
            POPUP_W,
            POPUP_H,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return Err("CreateWindowExW failed".into());
        }

        STATE.with(|state| state.borrow_mut().hwnd = hwnd);

        if RegisterHotKey(
            hwnd,
            HOTKEY_ID,
            MOD_WIN | MOD_SHIFT | MOD_NOREPEAT,
            VK_OEM_PERIOD as u32,
        ) == 0
        {
            DestroyWindow(hwnd);
            return Err(
                "Could not register Win+Shift+. . Another application may already own this hotkey."
                    .into(),
            );
        }

        if !add_tray_icon(hwnd) {
            UnregisterHotKey(hwnd, HOTKEY_ID);
            DestroyWindow(hwnd);
            return Err("Could not create the system tray icon".into());
        }

        let mut msg: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result == -1 {
                break;
            }
            if result == 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

pub fn show_error(message: &str) {
    unsafe {
        let text = wide(message);
        let title = wide(APP_NAME);
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

