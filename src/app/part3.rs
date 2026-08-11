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

    // Programmatic focus transfer is explicitly suppressed so WM_ACTIVATE can
    // still close the picker for a real click outside the window.
    SUPPRESS_DEACTIVATE.store(true, Ordering::SeqCst);
    SetForegroundWindow(target);
    thread::sleep(Duration::from_millis(24));

    if GetForegroundWindow() != target {
        SUPPRESS_DEACTIVATE.store(false, Ordering::SeqCst);
        return;
    }

    let pasted = if should_use_clipboard(&content) {
        paste_unicode_text(hwnd, &content)
    } else {
        send_unicode_text(&content);
        true
    };

    // Clipboard paste is asynchronous in some applications. Give the target a
    // short window to consume WM_PASTE before restoring picker focus.
    thread::sleep(Duration::from_millis(if pasted && should_use_clipboard(&content) { 70 } else { 18 }));
    SetForegroundWindow(hwnd);
    SetFocus(hwnd);
    thread::sleep(Duration::from_millis(8));
    SUPPRESS_DEACTIVATE.store(false, Ordering::SeqCst);
    InvalidateRect(hwnd, null(), 0);
}

fn should_use_clipboard(text: &str) -> bool {
    text.contains('\n') || text.contains('\r') || text.encode_utf16().count() > 48
}

unsafe fn paste_unicode_text(owner: HWND, text: &str) -> bool {
    // CF_UNICODETEXT is conventionally CRLF-delimited on Windows. Normalize
    // line endings so multiline snippets and ASCII art keep their structure.
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\r\n");
    let mut utf16: Vec<u16> = normalized.encode_utf16().collect();
    utf16.push(0);
    let byte_len = utf16.len() * size_of::<u16>();

    let memory = GlobalAlloc(GMEM_MOVEABLE, byte_len);
    if memory.is_null() { return false; }

    let destination = GlobalLock(memory) as *mut u16;
    if destination.is_null() {
        GlobalFree(memory);
        return false;
    }
    std::ptr::copy_nonoverlapping(utf16.as_ptr(), destination, utf16.len());
    GlobalUnlock(memory);

    if OpenClipboard(owner) == 0 {
        GlobalFree(memory);
        return false;
    }
    EmptyClipboard();
    let stored = SetClipboardData(CF_UNICODETEXT_FORMAT, memory as HANDLE);
    CloseClipboard();
    if stored.is_null() {
        GlobalFree(memory);
        return false;
    }

    send_ctrl_v();
    true
}

unsafe fn send_ctrl_v() {
    let mut inputs = [zeroed::<INPUT>(); 4];

    inputs[0].r#type = INPUT_KEYBOARD;
    inputs[0].Anonymous.ki = KEYBDINPUT {
        wVk: VK_CONTROL,
        wScan: 0,
        dwFlags: 0,
        time: 0,
        dwExtraInfo: 0,
    };
    inputs[1].r#type = INPUT_KEYBOARD;
    inputs[1].Anonymous.ki = KEYBDINPUT {
        wVk: b'V' as u16,
        wScan: 0,
        dwFlags: 0,
        time: 0,
        dwExtraInfo: 0,
    };
    inputs[2].r#type = INPUT_KEYBOARD;
    inputs[2].Anonymous.ki = KEYBDINPUT {
        wVk: b'V' as u16,
        wScan: 0,
        dwFlags: KEYEVENTF_KEYUP,
        time: 0,
        dwExtraInfo: 0,
    };
    inputs[3].r#type = INPUT_KEYBOARD;
    inputs[3].Anonymous.ki = KEYBDINPUT {
        wVk: VK_CONTROL,
        wScan: 0,
        dwFlags: KEYEVENTF_KEYUP,
        time: 0,
        dwExtraInfo: 0,
    };

    SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32);
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
