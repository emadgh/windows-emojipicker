pub(crate) fn preview_is_picker_hwnd(hwnd: HWND) -> bool {
    STATE.with(|cell| cell.borrow().hwnd == hwnd)
}

pub(crate) fn preview_select_at_client(x: i32, y: i32) -> Option<(ItemKind, String, String)> {
    let entry = STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let position = hit_test_item(x, y, &state)?;
        state.selected = position;
        state.hovered = Some(position);
        state.filtered.get(position).copied()
    })?;

    preview_payload_for_entry(entry)
}

pub(crate) fn preview_hover_payload() -> Option<(ItemKind, String, String)> {
    let entry = STATE.with(|cell| {
        let state = cell.borrow();
        let position = state.hovered?;
        state.filtered.get(position).copied()
    })?;

    preview_payload_for_entry(entry)
}

pub(crate) fn preview_selected_payload() -> Option<(ItemKind, String, String)> {
    let entry = STATE.with(|cell| {
        let state = cell.borrow();
        state.filtered.get(state.selected).copied()
    })?;

    preview_payload_for_entry(entry)
}

fn preview_payload_for_entry(entry: CatalogRef) -> Option<(ItemKind, String, String)> {
    match entry {
        CatalogRef::Builtin(index) => {
            let item = items().get(index)?;
            Some((item.kind, item.title.to_string(), item.content.to_string()))
        }
        CatalogRef::Custom(index) => {
            let item = custom::get(index)?;
            Some((item.kind, item.title, item.content))
        }
    }
}
