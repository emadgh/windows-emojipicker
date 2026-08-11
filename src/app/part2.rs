unsafe fn open_picker(hwnd: HWND, origin: OpenOrigin) {
    let target = GetForegroundWindow();
    let (x, y) = match origin {
        // Match GahYar exactly for an actual tray click: the cursor is sitting
        // on the icon, so center the popup on that X coordinate and place it
        // at the bottom of the monitor work area.
        OpenOrigin::Tray => WINDOW_BASE.above_taskbar_at_cursor(8),
        OpenOrigin::Hotkey => caret_position(target)
            .map(|point| unsafe { WINDOW_BASE.near_anchor(point, 10) })
            .or_else(|| unsafe { WINDOW_BASE.above_tray_icon(hwnd, TRAY_ICON_ID, 8) })
            .unwrap_or_else(|| unsafe { WINDOW_BASE.above_taskbar_at_cursor(8) }),
    };

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

    WINDOW_BASE.apply_rounding(hwnd);
    SetWindowPos(hwnd, HWND_TOPMOST, x, y, POPUP_W, POPUP_H, SWP_SHOWWINDOW);
    SetForegroundWindow(hwnd);
    SetFocus(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn caret_position(target: HWND) -> Option<POINT> {
    // UI Automation TextPattern2 is the primary path because many modern apps
    // do not expose their caret through the legacy GUITHREADINFO structure.
    if let Some((x, y)) = caret::focused_caret_point() {
        let point = POINT { x, y };
        if anchor_is_usable(point, target) {
            return Some(point);
        }
    }

    // Native/legacy edit controls generally expose an exact system caret.
    if !target.is_null() {
        let thread_id = GetWindowThreadProcessId(target, null_mut());
        if thread_id != 0 {
            let mut info: GUITHREADINFO = zeroed();
            info.cbSize = size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(thread_id, &mut info) != 0 && !info.hwndCaret.is_null() {
                let mut point = POINT { x: info.rcCaret.left, y: info.rcCaret.bottom };
                if ClientToScreen(info.hwndCaret, &mut point) != 0
                    && anchor_is_usable(point, target)
                {
                    return Some(point);
                }
            }
        }
    }

    None
}

unsafe fn anchor_is_usable(point: POINT, target: HWND) -> bool {
    if point.x == 0 && point.y == 0 {
        return false;
    }

    if MonitorFromPoint(point, MONITOR_DEFAULTTONULL).is_null() {
        return false;
    }

    if target.is_null() {
        return true;
    }

    let mut rect: RECT = zeroed();
    if GetWindowRect(target, &mut rect) == 0 || !rect_is_usable(rect) {
        return true;
    }

    const MARGIN: i32 = 96;
    point.x >= rect.left.saturating_sub(MARGIN)
        && point.x <= rect.right.saturating_add(MARGIN)
        && point.y >= rect.top.saturating_sub(MARGIN)
        && point.y <= rect.bottom.saturating_add(MARGIN)
}

fn rect_is_usable(rect: RECT) -> bool {
    rect.right > rect.left && rect.bottom > rect.top
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
