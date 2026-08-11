unsafe fn insert_selected(hwnd: HWND) {
    let selection = STATE.with(|cell| {
        let state = cell.borrow();
        let entry = state.filtered.get(state.selected).copied()?;
        let content = match entry {
            CatalogRef::Builtin(index) => items().get(index)?.content.to_string(),
            CatalogRef::Custom(index) => custom::get(index)?.content,
        };
        Some((state.target_hwnd, content))
    });

    let Some((target, content)) = selection else { return; };
    if target.is_null() || IsWindow(target) == 0 { return; }

    // Keep the picker visible. Focus the captured target only for the short
    // injection window, then return keyboard focus to the picker so multiple
    // emoji/items can be sent consecutively without reopening it.
    SetForegroundWindow(target);
    thread::sleep(Duration::from_millis(18));

    if GetForegroundWindow() != target {
        return;
    }

    send_unicode_text(&content);
    thread::sleep(Duration::from_millis(12));
    SetForegroundWindow(hwnd);
    SetFocus(hwnd);
    InvalidateRect(hwnd, null(), 0);
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
        SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32);
    }
}

unsafe fn hit_test_category(hwnd: HWND, x: i32, y: i32) -> bool {
    if !(TABS_Y..TABS_Y + TABS_H).contains(&y) { return false; }
    if x < PAD || x >= POPUP_W - PAD { return false; }

    let width = (POPUP_W - PAD * 2) / ItemKind::ALL.len() as i32;
    let index = ((x - PAD) / width) as usize;
    if index >= ItemKind::ALL.len() { return false; }

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

unsafe fn hit_test_action(hwnd: HWND, x: i32, y: i32) -> bool {
    if !(ACTION_Y..ACTION_Y + ACTION_H).contains(&y) { return false; }
    if (PAD..104).contains(&x) {
        manager::show(hwnd, WM_CUSTOM_CHANGED);
        return true;
    }
    if (112..190).contains(&x) {
        about::show(hwnd, WM_REQUEST_UPDATE);
        return true;
    }
    if (198..302).contains(&x) {
        request_manual_update(hwnd);
        return true;
    }
    if (310..POPUP_W - PAD).contains(&x) {
        toggle_theme(hwnd);
        return true;
    }
    false
}
