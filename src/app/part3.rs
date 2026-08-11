unsafe fn handle_keydown(hwnd: HWND, wparam: WPARAM) -> bool {
    let key = wparam as u16;

    match key {
        VK_ESCAPE => {
            ShowWindow(hwnd, SW_HIDE);
            true
        }
        VK_RETURN => {
            insert_selected(hwnd);
            true
        }
        VK_TAB => {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                state.category_index = (state.category_index + 1) % ItemKind::ALL.len();
                state.selected = 0;
                state.scroll = 0;
                state.hovered = None;
            });
            InvalidateRect(hwnd, null(), 0);
            true
        }
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END => {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                let count = filtered_indices(&state).len();
                if count == 0 {
                    state.selected = 0;
                    state.scroll = 0;
                    return;
                }

                state.selected = state.selected.min(count - 1);
                match key {
                    VK_LEFT => state.selected = state.selected.saturating_sub(1),
                    VK_RIGHT => state.selected = (state.selected + 1).min(count - 1),
                    VK_UP => state.selected = state.selected.saturating_sub(COLS),
                    VK_DOWN => state.selected = (state.selected + COLS).min(count - 1),
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
        0x08 => {
            STATE.with(|cell| pop_utf16_scalar(&mut cell.borrow_mut().query_utf16));
        }
        0x09 | 0x0d | 0x1b => return true,
        0x20..=0xffff => {
            STATE.with(|cell| cell.borrow_mut().query_utf16.push(unit));
        }
        _ => return false,
    }

    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.selected = 0;
        state.scroll = 0;
        state.hovered = None;
    });
    InvalidateRect(hwnd, null(), 0);
    true
}

fn pop_utf16_scalar(value: &mut Vec<u16>) {
    if let Some(last) = value.pop() {
        if (0xdc00..=0xdfff).contains(&last) {
            if let Some(previous) = value.last() {
                if (0xd800..=0xdbff).contains(previous) {
                    value.pop();
                }
            }
        }
    }
}

unsafe fn insert_selected(hwnd: HWND) {
    let selection = STATE.with(|cell| {
        let state = cell.borrow();
        let filtered = filtered_indices(&state);
        let item = filtered
            .get(state.selected)
            .and_then(|index| ITEMS.get(*index))
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
    });
    InvalidateRect(hwnd, null(), 0);
    true
}

fn hit_test_item(x: i32, y: i32, state: &AppState) -> Option<usize> {
    if y < CONTENT_TOP || y >= FOOTER_Y - 8 {
        return None;
    }

    let cell_w = card_width();
    let relative_x = x - PAD;
    let relative_y = y - CONTENT_TOP;
    if relative_x < 0 || relative_y < 0 {
        return None;
    }

    let stride_x = cell_w + GAP;
    let stride_y = CARD_H + GAP;
    let col = (relative_x / stride_x) as usize;
    let row = (relative_y / stride_y) as usize;
    if col >= COLS || row >= VISIBLE_ROWS {
        return None;
    }

    if relative_x % stride_x >= cell_w || relative_y % stride_y >= CARD_H {
        return None;
    }

    let position = state.scroll + row * COLS + col;
    if position < filtered_indices(state).len() {
        Some(position)
    } else {
        None
    }
}

fn filtered_indices(state: &AppState) -> Vec<usize> {
    let query = String::from_utf16_lossy(&state.query_utf16)
        .trim()
        .to_lowercase();
    let category = ItemKind::ALL[state.category_index];

    ITEMS
        .iter()
        .enumerate()
        .filter(|(_, item)| match category {
            Some(kind) => item.kind == kind,
            None => true,
        })
        .filter(|(_, item)| {
            if query.is_empty() {
                return true;
            }
            item.title.to_lowercase().contains(&query)
                || item.content.to_lowercase().contains(&query)
                || item.keywords.to_lowercase().contains(&query)
        })
        .map(|(index, _)| index)
        .collect()
}

fn ensure_selected_visible(state: &mut AppState, count: usize) {
    let visible = COLS * VISIBLE_ROWS;
    if state.selected < state.scroll {
        state.scroll = (state.selected / COLS) * COLS;
    } else if state.selected >= state.scroll + visible {
        let selected_row = state.selected / COLS;
        state.scroll = selected_row
            .saturating_sub(VISIBLE_ROWS - 1)
            .saturating_mul(COLS);
    }
    state.scroll = state.scroll.min(max_scroll(count));
}

fn max_scroll(count: usize) -> usize {
    let rows = (count + COLS - 1) / COLS;
    rows.saturating_sub(VISIBLE_ROWS) * COLS
}

fn card_width() -> i32 {
    (POPUP_W - PAD * 2 - GAP * (COLS as i32 - 1)) / COLS as i32
}

