use std::{
    cell::RefCell,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    thread,
    time::Duration,
};

use windows_sys::{
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
    data::items,
    model::{ItemKind, PickerItem},
    renderer::{self, EmojiDraw},
};

const APP_NAME: &str = "Windows Emoji Picker";
const WINDOW_CLASS: &str = "WindowsEmojiPicker.NativeWindow";

const HOTKEY_ID: i32 = 1;
const TRAY_ICON_ID: u32 = 1;
const WM_TRAY: u32 = WM_APP + 1;
const CMD_OPEN: usize = 1001;
const CMD_EXIT: usize = 1002;

const POPUP_W: i32 = 440;
const POPUP_H: i32 = 440;
const PAD: i32 = 12;
const SEARCH_Y: i32 = 12;
const SEARCH_H: i32 = 44;
const TABS_Y: i32 = 64;
const TABS_H: i32 = 32;
const CONTENT_TOP: i32 = 108;
const FOOTER_Y: i32 = 408;
const FOOTER_H: i32 = 24;
const STANDARD_COLS: usize = 2;
const STANDARD_ROWS: usize = 4;
const STANDARD_GAP: i32 = 8;
const STANDARD_CARD_H: i32 = 64;
const EMOJI_COLS: usize = 8;
const EMOJI_ROWS: usize = 6;
const EMOJI_GAP: i32 = 5;
const EMOJI_CARD_H: i32 = 44;

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
    filtered: Vec<usize>,
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
            filtered: Vec::new(),
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

        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            state.hwnd = hwnd;
            rebuild_filter(&mut state);
        });

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

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY if wparam == HOTKEY_ID as usize => {
            open_picker(hwnd);
            return 0;
        }
        WM_TRAY => {
            match lparam as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => open_picker(hwnd),
                WM_RBUTTONUP | WM_CONTEXTMENU => show_tray_menu(hwnd),
                _ => {}
            }
            return 0;
        }
        WM_COMMAND => {
            match wparam & 0xffff {
                CMD_OPEN => open_picker(hwnd),
                CMD_EXIT => {
                    DestroyWindow(hwnd);
                }
                _ => {}
            }
            return 0;
        }
        WM_ACTIVATE => {
            if (wparam & 0xffff) as u32 == WA_INACTIVE {
                ShowWindow(hwnd, SW_HIDE);
            }
            return 0;
        }
        WM_KEYDOWN => {
            if handle_keydown(hwnd, wparam) {
                return 0;
            }
        }
        WM_CHAR => {
            if handle_char(hwnd, wparam) {
                return 0;
            }
        }
        WM_MOUSEMOVE => {
            let (x, y) = point_from_lparam(lparam);
            let changed = STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                let new_hover = hit_test_item(x, y, &state);
                if state.hovered != new_hover {
                    state.hovered = new_hover;
                    true
                } else {
                    false
                }
            });
            if changed {
                InvalidateRect(hwnd, null(), 0);
            }
            return 0;
        }
        WM_LBUTTONUP => {
            let (x, y) = point_from_lparam(lparam);
            if hit_test_category(hwnd, x, y) {
                return 0;
            }

            let clicked = STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                let hit = hit_test_item(x, y, &state);
                if let Some(position) = hit {
                    state.selected = position;
                }
                hit.is_some()
            });

            if clicked {
                insert_selected(hwnd);
            }
            return 0;
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xffff) as u16 as i16;
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                let layout = grid_layout(&state);
                let max_scroll = max_scroll(state.filtered.len(), layout);
                if delta < 0 {
                    state.scroll = (state.scroll + layout.cols).min(max_scroll);
                } else {
                    state.scroll = state.scroll.saturating_sub(layout.cols);
                }
            });
            InvalidateRect(hwnd, null(), 0);
            return 0;
        }
        WM_PAINT => {
            paint(hwnd);
            return 0;
        }
        WM_ERASEBKGND => return 1,
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            return 0;
        }
        WM_DESTROY => {
            remove_tray_icon(hwnd);
            UnregisterHotKey(hwnd, HOTKEY_ID);
            PostQuitMessage(0);
            return 0;
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

