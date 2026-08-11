unsafe fn open_picker(hwnd: HWND, origin: OpenOrigin) {
    let target = GetForegroundWindow();
    let (x, y) = match origin {
        OpenOrigin::Tray => tray_popup_position(hwnd),
        OpenOrigin::Hotkey => caret_position(target)
            .map(popup_position)
            .unwrap_or_else(|| tray_popup_position(hwnd)),
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

    let region = CreateRoundRectRgn(0, 0, POPUP_W + 1, POPUP_H + 1, 18, 18);
    SetWindowRgn(hwnd, region, 1);
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

    // Do not guess from the active window or mouse. If a real caret cannot be
    // resolved, the caller deliberately falls back to the system-tray icon.
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

    // UIA can occasionally return a stale caret from a previously focused app.
    // Requiring the anchor to be near the active top-level window prevents that.
    const MARGIN: i32 = 96;
    point.x >= rect.left.saturating_sub(MARGIN)
        && point.x <= rect.right.saturating_add(MARGIN)
        && point.y >= rect.top.saturating_sub(MARGIN)
        && point.y <= rect.bottom.saturating_add(MARGIN)
}

fn rect_is_usable(rect: RECT) -> bool {
    rect.right > rect.left && rect.bottom > rect.top
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

unsafe fn tray_popup_position(hwnd: HWND) -> (i32, i32) {
    const GAP: i32 = 8;

    if let Some(icon) = tray_icon_rect(hwnd) {
        let center = POINT {
            x: icon.left + (icon.right - icon.left) / 2,
            y: icon.top + (icon.bottom - icon.top) / 2,
        };
        let monitor = MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST);
        if !monitor.is_null() {
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info) != 0 {
                let monitor_rect = info.rcMonitor;
                let work = info.rcWork;
                let (mut x, mut y) = if work.bottom < monitor_rect.bottom {
                    // Standard bottom taskbar: open directly above the tray icon.
                    (center.x - POPUP_W / 2, icon.top - POPUP_H - GAP)
                } else if work.top > monitor_rect.top {
                    // Top taskbar.
                    (center.x - POPUP_W / 2, icon.bottom + GAP)
                } else if work.right < monitor_rect.right {
                    // Right taskbar.
                    (icon.left - POPUP_W - GAP, center.y - POPUP_H / 2)
                } else if work.left > monitor_rect.left {
                    // Left taskbar.
                    (icon.right + GAP, center.y - POPUP_H / 2)
                } else {
                    // Auto-hidden/overlay taskbar: Windows normally keeps the tray
                    // along the bottom edge, so prefer the same visual placement.
                    (center.x - POPUP_W / 2, icon.top - POPUP_H - GAP)
                };

                clamp_popup_to_work_area(&mut x, &mut y, work);
                return (x, y);
            }
        }
    }

    // Extremely defensive fallback for Explorer restarts or a temporarily
    // unavailable tray rect. Keep the picker by the taskbar side of the primary
    // work area instead of placing it at a screen corner chosen from a caret guess.
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
    let mut work = RECT {
        left: 0,
        top: 0,
        right: GetSystemMetrics(SM_CXSCREEN),
        bottom: GetSystemMetrics(SM_CYSCREEN),
    };
    if !monitor.is_null() {
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            work = info.rcWork;
        }
    }

    (
        (work.right - POPUP_W - 12).max(work.left),
        (work.bottom - POPUP_H - 12).max(work.top),
    )
}

fn clamp_popup_to_work_area(x: &mut i32, y: &mut i32, work: RECT) {
    let max_x = (work.right - POPUP_W).max(work.left);
    let max_y = (work.bottom - POPUP_H).max(work.top);
    *x = (*x).clamp(work.left, max_x);
    *y = (*y).clamp(work.top, max_y);
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
