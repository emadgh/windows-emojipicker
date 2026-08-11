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
                let count = filtered_indices(&state).len();
                let max_scroll = max_scroll(count);
                if delta < 0 {
                    state.scroll = (state.scroll + COLS).min(max_scroll);
                } else {
                    state.scroll = state.scroll.saturating_sub(COLS);
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

unsafe fn open_picker(hwnd: HWND) {
    let target = GetForegroundWindow();
    let anchor = caret_or_cursor_position(target);
    let (x, y) = popup_position(anchor);

    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if !target.is_null() && target != hwnd {
            state.target_hwnd = target;
        }
        state.query_utf16.clear();
        state.selected = 0;
        state.scroll = 0;
        state.hovered = None;
    });

    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        x,
        y,
        POPUP_W,
        POPUP_H,
        SWP_SHOWWINDOW,
    );
    SetForegroundWindow(hwnd);
    SetFocus(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn caret_or_cursor_position(target: HWND) -> POINT {
    if !target.is_null() {
        let thread_id = GetWindowThreadProcessId(target, null_mut());
        if thread_id != 0 {
            let mut info: GUITHREADINFO = zeroed();
            info.cbSize = size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(thread_id, &mut info) != 0 && !info.hwndCaret.is_null() {
                let mut point = POINT {
                    x: info.rcCaret.left,
                    y: info.rcCaret.bottom,
                };
                if ClientToScreen(info.hwndCaret, &mut point) != 0 {
                    return point;
                }
            }
        }
    }

    let mut point: POINT = zeroed();
    GetCursorPos(&mut point);
    point
}

unsafe fn popup_position(anchor: POINT) -> (i32, i32) {
    let monitor = MonitorFromPoint(anchor, MONITOR_DEFAULTTONEAREST);
    let mut work = RECT {
        left: 0,
        top: 0,
        right: anchor.x + POPUP_W,
        bottom: anchor.y + POPUP_H,
    };

    if !monitor.is_null() {
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            work = info.rcWork;
        }
    }

    let mut x = anchor.x;
    let mut y = anchor.y + 10;

    if x + POPUP_W > work.right {
        x = work.right - POPUP_W;
    }
    if x < work.left {
        x = work.left;
    }

    if y + POPUP_H > work.bottom {
        y = anchor.y - POPUP_H - 10;
    }
    if y < work.top {
        y = work.top;
    }

    (x, y)
}

