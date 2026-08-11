use std::{
    cell::RefCell,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{
        DataExchange::*,
        LibraryLoader::*,
        Memory::*,
    },
    UI::{
        Input::KeyboardAndMouse::*,
        Shell::*,
        WindowsAndMessaging::*,
    },
};

use crate::{
    about,
    caret,
    custom,
    data::items,
    manager,
    model::{ItemKind, PickerItem},
    renderer::{self, EmojiDraw},
    settings,
    theme::Palette,
    update,
};

const APP_NAME: &str = "Windows Emoji Picker";
const WINDOW_CLASS: &str = "WindowsEmojiPicker.NativeWindow";
const APP_ICON_ID: usize = 1;

const HOTKEY_ID: i32 = 1;
const TRAY_ICON_ID: u32 = 1;
const WM_TRAY: u32 = WM_APP + 1;
const WM_CUSTOM_CHANGED: u32 = WM_APP + 2;
const WM_UPDATE_STATUS: u32 = WM_APP + 3;
const WM_APPLY_UPDATE: u32 = WM_APP + 4;
const WM_REQUEST_UPDATE: u32 = WM_APP + 5;
const CMD_OPEN: usize = 1001;
const CMD_MANAGE: usize = 1002;
const CMD_ABOUT: usize = 1003;
const CMD_UPDATE: usize = 1004;
const CMD_THEME: usize = 1005;
const CMD_AUTO_UPDATE: usize = 1006;
const CMD_EXIT: usize = 1007;
const UPDATE_CHECK_TIMER_ID: usize = 1;
const UPDATE_CHECK_INTERVAL_MS: u32 = 6 * 60 * 60 * 1000;
const CF_UNICODETEXT_FORMAT: u32 = 13;

const POPUP_W: i32 = 430;
const POPUP_H: i32 = 478;
const PAD: i32 = 12;
const SEARCH_Y: i32 = 12;
const SEARCH_H: i32 = 44;
const TABS_Y: i32 = 64;
const TABS_H: i32 = 32;
const CONTENT_TOP: i32 = 108;
const ACTION_Y: i32 = 408;
const ACTION_H: i32 = 32;
const FOOTER_Y: i32 = 444;
const FOOTER_H: i32 = 26;
const STANDARD_COLS: usize = 2;
const STANDARD_ROWS: usize = 4;
const STANDARD_GAP: i32 = 8;
const STANDARD_CARD_H: i32 = 64;
const DENSE_COLS: usize = 8;
const DENSE_ROWS: usize = 6;
const DENSE_GAP: i32 = 5;
const DENSE_CARD_H: i32 = 44;

static MANUAL_UPDATE_REQUEST: AtomicBool = AtomicBool::new(false);
static SUPPRESS_DEACTIVATE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug)]
enum CatalogRef {
    Builtin(usize),
    Custom(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenOrigin {
    Hotkey,
    Tray,
}

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
    filtered: Vec<CatalogRef>,
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

        let _ = settings::get();
        let _ = custom::snapshot();

        let class_name = wide(WINDOW_CLASS);
        let window_title = wide(APP_NAME);
        let icon = load_app_icon();
        let cursor = LoadCursorW(null_mut(), IDC_ARROW);

        let mut wc: WNDCLASSW = zeroed();
        wc.style = 0;
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
                "Could not register Win+Shift+.. Another application may already own this hotkey."
                    .into(),
            );
        }

        if !add_tray_icon(hwnd) {
            UnregisterHotKey(hwnd, HOTKEY_ID);
            DestroyWindow(hwnd);
            return Err("Could not create the system tray icon".into());
        }

        SetTimer(hwnd, UPDATE_CHECK_TIMER_ID, UPDATE_CHECK_INTERVAL_MS, None);
        update::start_check(hwnd, WM_UPDATE_STATUS);

        let mut msg: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result <= 0 { break; }
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
        MessageBoxW(null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
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
            if IsWindowVisible(hwnd) != 0 {
                ShowWindow(hwnd, SW_HIDE);
            } else {
                open_picker(hwnd, OpenOrigin::Hotkey);
            }
            return 0;
        }
        WM_TRAY => {
            match lparam as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => open_picker(hwnd, OpenOrigin::Tray),
                WM_RBUTTONUP | WM_CONTEXTMENU => show_tray_menu(hwnd),
                _ => {}
            }
            return 0;
        }
        WM_COMMAND => {
            match wparam & 0xffff {
                CMD_OPEN => open_picker(hwnd, OpenOrigin::Tray),
                CMD_MANAGE => manager::show(hwnd, WM_CUSTOM_CHANGED),
                CMD_ABOUT => about::show(hwnd, WM_REQUEST_UPDATE),
                CMD_UPDATE => request_manual_update(hwnd),
                CMD_THEME => toggle_theme(hwnd),
                CMD_AUTO_UPDATE => toggle_auto_update(hwnd),
                CMD_EXIT => { DestroyWindow(hwnd); }
                _ => {}
            }
            return 0;
        }
        WM_ACTIVATE => {
            if (wparam & 0xffff) as u32 == WA_INACTIVE
                && !SUPPRESS_DEACTIVATE.load(Ordering::SeqCst)
            {
                ShowWindow(hwnd, SW_HIDE);
            }
            return 0;
        }
        WM_CUSTOM_CHANGED => {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                state.selected = 0;
                state.scroll = 0;
                state.hovered = None;
                rebuild_filter(&mut state);
            });
            InvalidateRect(hwnd, null(), 0);
            return 0;
        }
        WM_REQUEST_UPDATE => {
            request_manual_update(hwnd);
            return 0;
        }
        WM_UPDATE_STATUS => {
            let status = update::status();
            let manual = MANUAL_UPDATE_REQUEST.load(Ordering::SeqCst);
            if matches!(status, update::UpdateStatus::Available(_))
                && (settings::get().auto_update || manual)
                && update::start_download(hwnd, WM_UPDATE_STATUS, WM_APPLY_UPDATE)
            {
                MANUAL_UPDATE_REQUEST.store(false, Ordering::SeqCst);
            } else if matches!(status, update::UpdateStatus::UpToDate | update::UpdateStatus::Idle | update::UpdateStatus::Failed(_)) {
                MANUAL_UPDATE_REQUEST.store(false, Ordering::SeqCst);
            }
            about::invalidate();
            InvalidateRect(hwnd, null(), 0);
            return 0;
        }
        WM_APPLY_UPDATE => {
            DestroyWindow(hwnd);
            return 0;
        }
        WM_TIMER if wparam == UPDATE_CHECK_TIMER_ID => {
            update::start_check(hwnd, WM_UPDATE_STATUS);
            return 0;
        }
        WM_KEYDOWN => {
            if handle_keydown(hwnd, wparam) { return 0; }
        }
        WM_CHAR => {
            if handle_char(hwnd, wparam) { return 0; }
        }
        WM_NCHITTEST => {
            let (screen_x, screen_y) = point_from_lparam(lparam);
            let mut point = POINT { x: screen_x, y: screen_y };
            if ScreenToClient(hwnd, &mut point) != 0 {
                if is_draggable_background(point.x, point.y) {
                    return HTCAPTION as LRESULT;
                }
                if (0..POPUP_W).contains(&point.x) && (0..POPUP_H).contains(&point.y) {
                    return HTCLIENT as LRESULT;
                }
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
                } else { false }
            });
            if changed { InvalidateRect(hwnd, null(), 0); }
            return 0;
        }
        WM_LBUTTONUP => {
            let (x, y) = point_from_lparam(lparam);
            if hit_test_action(hwnd, x, y) || hit_test_category(hwnd, x, y) { return 0; }

            let clicked = STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                let hit = hit_test_item(x, y, &state);
                if let Some(position) = hit { state.selected = position; }
                hit.is_some()
            });
            if clicked { insert_selected(hwnd); }
            return 0;
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xffff) as u16 as i16;
            let changed = STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                let old = state.scroll;
                let layout = grid_layout(&state);
                let max_scroll = max_scroll(state.filtered.len(), layout);
                if delta < 0 { state.scroll = (state.scroll + layout.cols).min(max_scroll); }
                else { state.scroll = state.scroll.saturating_sub(layout.cols); }
                old != state.scroll
            });
            if changed { InvalidateRect(hwnd, null(), 0); }
            return 0;
        }
        WM_PAINT => { paint(hwnd); return 0; }
        WM_ERASEBKGND => return 1,
        WM_CLOSE => { ShowWindow(hwnd, SW_HIDE); return 0; }
        WM_DESTROY => {
            KillTimer(hwnd, UPDATE_CHECK_TIMER_ID);
            remove_tray_icon(hwnd);
            UnregisterHotKey(hwnd, HOTKEY_ID);
            PostQuitMessage(0);
            return 0;
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn is_draggable_background(x: i32, y: i32) -> bool {
    if !(0..POPUP_W).contains(&x) || !(0..POPUP_H).contains(&y) {
        return false;
    }

    if (SEARCH_Y..SEARCH_Y + SEARCH_H).contains(&y)
        && (PAD..POPUP_W - PAD).contains(&x)
    {
        return false;
    }

    if (TABS_Y..TABS_Y + TABS_H).contains(&y)
        && (PAD..POPUP_W - PAD).contains(&x)
    {
        return false;
    }

    if STATE.with(|cell| hit_test_item(x, y, &cell.borrow()).is_some()) {
        return false;
    }

    if (ACTION_Y..ACTION_Y + ACTION_H).contains(&y)
        && ((PAD..104).contains(&x)
            || (112..190).contains(&x)
            || (198..302).contains(&x)
            || (310..POPUP_W - PAD).contains(&x))
    {
        return false;
    }

    true
}

unsafe fn request_manual_update(hwnd: HWND) {
    MANUAL_UPDATE_REQUEST.store(true, Ordering::SeqCst);
    match update::status() {
        update::UpdateStatus::Available(_) | update::UpdateStatus::Failed(_) => {
            if update::start_download(hwnd, WM_UPDATE_STATUS, WM_APPLY_UPDATE) {
                MANUAL_UPDATE_REQUEST.store(false, Ordering::SeqCst);
            }
        }
        update::UpdateStatus::Checking | update::UpdateStatus::Downloading => {}
        _ => { update::start_check(hwnd, WM_UPDATE_STATUS); }
    }
    about::invalidate();
}

unsafe fn toggle_auto_update(hwnd: HWND) {
    let enabled = !settings::get().auto_update;
    settings::set_auto_update(enabled);
    if enabled {
        match update::status() {
            update::UpdateStatus::Available(_) | update::UpdateStatus::Failed(_) => {
                update::start_download(hwnd, WM_UPDATE_STATUS, WM_APPLY_UPDATE);
            }
            update::UpdateStatus::Idle => { update::start_check(hwnd, WM_UPDATE_STATUS); }
            _ => {}
        }
    }
    about::invalidate();
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn toggle_theme(hwnd: HWND) {
    settings::toggle_theme();
    InvalidateRect(hwnd, null(), 0);
    manager::invalidate();
    about::invalidate();
}
