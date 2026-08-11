unsafe fn insert_selected(hwnd: HWND) {
    let selection = STATE.with(|cell| {
        let state = cell.borrow();
        let item = state.filtered
            .get(state.selected)
            .and_then(|index| items().get(*index))
            .copied();
        item.map(|item| (state.target_hwnd, item.content))
    });

    let Some((target, content)) = selection else {
        return;
    };

    ShowWindow(hwnd, SW_HIDE);

    if target.is_null() || IsWindow(target) == 0 {
        return;
    }

    SetForegroundWindow(target);
    thread::sleep(Duration::from_millis(25));

    if GetForegroundWindow() != target {
        // Never inject into an unexpected foreground window.
        return;
    }

    send_unicode_text(content);
}

unsafe fn send_unicode_text(text: &str) {
    let mut inputs = Vec::<INPUT>::with_capacity(text.encode_utf16().count() * 2);

    for unit in text.encode_utf16() {
        let mut down: INPUT = zeroed();
        down.r#type = INPUT_KEYBOARD;
        down.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: unit,
            dwFlags: KEYEVENTF_UNICODE,
            time: 0,
            dwExtraInfo: 0,
        };

        let mut up: INPUT = zeroed();
        up.r#type = INPUT_KEYBOARD;
        up.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: unit,
            dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };
        inputs.push(down);
        inputs.push(up);
    }

    if !inputs.is_empty() {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        );
    }
}

unsafe fn hit_test_category(hwnd: HWND, x: i32, y: i32) -> bool {
    if !(TABS_Y..TABS_Y + TABS_H).contains(&y) {
        return false;
    }
    if x < PAD || x >= POPUP_W - PAD {
        return false;
    }

    let width = (POPUP_W - PAD * 2) / ItemKind::ALL.len() as i32;
    let index = ((x - PAD) / width) as usize;
    if index >= ItemKind::ALL.len() {
        return false;
    }

    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.category_index = index;
        state.selected = 0;
        state.scroll = 0;
        state.hovered = None;
        rebuild_filter(&mut state);
    });
    InvalidateRect(hwnd, null(), 0);
    true
}

