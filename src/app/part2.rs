unsafe fn open_picker(hwnd: HWND) {
    let target = GetForegroundWindow();
    let anchor = caret_position(target);
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
        rebuild_filter(&mut state);
    });

    let region = CreateRoundRectRgn(0, 0, POPUP_W + 1, POPUP_H + 1, 18, 18);
    SetWindowRgn(hwnd, region, 1);
    SetWindowPos(hwnd, HWND_TOPMOST, x, y, POPUP_W, POPUP_H, SWP_SHOWWINDOW);
    SetForegroundWindow(hwnd);
    SetFocus(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn caret_position(target: HWND) -> POINT {
    // UI Automation TextPattern2 is the primary path because many modern apps
    // do not expose their caret through the legacy GUITHREADINFO structure.
    if let Some((x, y)) = caret::focused_caret_point() {
        return POINT { x, y };
    }

    // Native/legacy edit controls generally expose an exact system caret.
    if !target.is_null() {
        let thread_id = GetWindowThreadProcessId(target, null_mut());
        if thread_id != 0 {
            let mut info: GUITHREADINFO = zeroed();
            info.cbSize = size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(thread_id, &mut info) != 0 {
                if !info.hwndCaret.is_null() {
                    let mut point = POINT { x: info.rcCaret.left, y: info.rcCaret.bottom };
                    if ClientToScreen(info.hwndCaret, &mut point) != 0 {
                        return point;
                    }
                }

                // If the application does not expose a caret, stay anchored to
                // the focused control rather than jumping to the mouse pointer.
                if !info.hwndFocus.is_null() {
                    let mut rect: RECT = zeroed();
                    if GetWindowRect(info.hwndFocus, &mut rect) != 0 {
                        return POINT { x: rect.left + 14, y: rect.bottom.min(rect.top + 40) };
                    }
                }
            }
        }

        let mut rect: RECT = zeroed();
        if GetWindowRect(target, &mut rect) != 0 {
            return POINT { x: rect.left + 24, y: rect.top + 48 };
        }
    }

    POINT { x: 32, y: 32 }
}

unsafe fn popup_position(anchor: POINT) -> (i32, i32) {
    let monitor = MonitorFromPoint(anchor, MONITOR_DEFAULTTONEAREST);
    let mut work = RECT { left: 0, top: 0, right: anchor.x + POPUP_W, bottom: anchor.y + POPUP_H };

    if !monitor.is_null() {
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) != 0 { work = info.rcWork; }
    }

    let mut x = anchor.x;
    let mut y = anchor.y + 10;
    if x + POPUP_W > work.right { x = work.right - POPUP_W; }
    if x < work.left { x = work.left; }
    if y + POPUP_H > work.bottom { y = anchor.y - POPUP_H - 10; }
    if y < work.top { y = work.top; }
    (x, y)
}

unsafe fn handle_keydown(hwnd: HWND, wparam: WPARAM) -> bool {
    let key = wparam as u16;

    match key {
        VK_ESCAPE => { ShowWindow(hwnd, SW_HIDE); true }
        VK_RETURN => { insert_selected(hwnd); true }
        VK_TAB => {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                state.category_index = (state.category_index + 1) % ItemKind::ALL.len();
                state.selected = 0;
                state.scroll = 0;
                state.hovered = None;
                rebuild_filter(&mut state);
            });
            InvalidateRect(hwnd, null(), 0);
            true
        }
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END => {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                let count = state.filtered.len();
                if count == 0 {
                    state.selected = 0;
                    state.scroll = 0;
                    return;
                }

                let layout = grid_layout(&state);
                state.selected = state.selected.min(count - 1);
                match key {
                    VK_LEFT => state.selected = state.selected.saturating_sub(1),
                    VK_RIGHT => state.selected = (state.selected + 1).min(count - 1),
                    VK_UP => state.selected = state.selected.saturating_sub(layout.cols),
                    VK_DOWN => state.selected = (state.selected + layout.cols).min(count - 1),
                    VK_HOME => state.selected = 0,
                    VK_END => state.selected = count - 1,
                    _ => {}
                }
                ensure_selected_visible(&mut state, count);
            });
            InvalidateRect(hwnd, null(), 0);
            true
        }
        _ => false,
    }
}

unsafe fn handle_char(hwnd: HWND, wparam: WPARAM) -> bool {
    let unit = wparam as u16;

    match unit {
        0x08 => { STATE.with(|cell| pop_utf16_scalar(&mut cell.borrow_mut().query_utf16)); }
        0x09 | 0x0d | 0x1b => return true,
        0x20..=0xffff => { STATE.with(|cell| cell.borrow_mut().query_utf16.push(unit)); }
        _ => return false,
    }

    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.selected = 0;
        state.scroll = 0;
        state.hovered = None;
        rebuild_filter(&mut state);
    });
    InvalidateRect(hwnd, null(), 0);
    true
}

fn pop_utf16_scalar(value: &mut Vec<u16>) {
    if let Some(last) = value.pop() {
        if (0xdc00..=0xdfff).contains(&last) {
            if let Some(previous) = value.last() {
                if (0xd800..=0xdbff).contains(previous) { value.pop(); }
            }
        }
    }
}
