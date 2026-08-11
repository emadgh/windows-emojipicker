unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    if screen.is_null() { return; }

    let mut client: RECT = zeroed();
    GetClientRect(hwnd, &mut client);

    // Draw the entire frame into a memory DC, including the DirectWrite emoji
    // overlay, and present it with one BitBlt. This avoids hover/scroll flash.
    let mem = CreateCompatibleDC(screen);
    let bitmap = if !mem.is_null() { CreateCompatibleBitmap(screen, client.right, client.bottom) } else { null_mut() };
    let buffered = !mem.is_null() && !bitmap.is_null();
    let old_bitmap = if buffered { SelectObject(mem, bitmap as _) } else { null_mut() };
    let hdc = if buffered { mem } else { screen };

    let palette = Palette::current();
    fill(hdc, &client, palette.background);
    SetBkMode(hdc, TRANSPARENT as i32);

    let normal_face = wide("Segoe UI");
    let symbol_face = wide("Segoe UI Symbol");
    let mono_face = wide("Consolas");
    let emoji_face = wide("Segoe UI Emoji");
    let normal_font = CreateFontW(-15, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, normal_face.as_ptr());
    let small_font = CreateFontW(-12, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, normal_face.as_ptr());
    let symbol_font = CreateFontW(-22, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, symbol_face.as_ptr());
    let mono_font = CreateFontW(-13, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, mono_face.as_ptr());
    let emoji_font = CreateFontW(-27, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, CLEARTYPE_QUALITY as u32, 0, emoji_face.as_ptr());

    let search_rect = RECT { left: PAD, top: SEARCH_Y, right: POPUP_W - PAD, bottom: SEARCH_Y + SEARCH_H };
    round_fill(hdc, search_rect, palette.surface, 12);
    let query = STATE.with(|cell| String::from_utf16_lossy(&cell.borrow().query_utf16));
    let mut text_rect = RECT { left: search_rect.left + 14, top: search_rect.top, right: search_rect.right - 12, bottom: search_rect.bottom };
    draw_text(
        hdc,
        normal_font,
        if query.is_empty() { "Search emoji, kaomoji, ASCII, text..." } else { &query },
        &mut text_rect,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        if query.is_empty() { palette.muted } else { palette.text },
    );

    let (category_index, selected, scroll, hovered, layout, visible_items) = STATE.with(|cell| {
        let state = cell.borrow();
        let layout = grid_layout(&state);
        let visible_items = state.filtered.iter().skip(state.scroll).take(layout.cols * layout.rows).copied().collect::<Vec<_>>();
        (state.category_index, state.selected, state.scroll, state.hovered, layout, visible_items)
    });

    let tab_width = (POPUP_W - PAD * 2) / ItemKind::ALL.len() as i32;
    for (index, kind) in ItemKind::ALL.iter().copied().enumerate() {
        let left = PAD + tab_width * index as i32;
        let right = if index + 1 == ItemKind::ALL.len() { POPUP_W - PAD } else { left + tab_width - 3 };
        let tab = RECT { left, top: TABS_Y, right, bottom: TABS_Y + TABS_H };
        round_fill(hdc, tab, if index == category_index { palette.accent } else { palette.surface }, 9);
        let mut label_rect = tab;
        draw_text(
            hdc, small_font, kind.label(), &mut label_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            if index == category_index { palette.accent_text } else { palette.muted },
        );
    }

    let cell_w = card_width(layout);
    let catalog = items();
    let mut emoji_draws = Vec::<EmojiDraw<'_>>::new();
    let mut emoji_fallbacks = Vec::<(&str, RECT)>::new();

    for (slot, entry) in visible_items.iter().copied().enumerate() {
        let position = scroll + slot;
        let row = slot / layout.cols;
        let col = slot % layout.cols;
        let x = PAD + col as i32 * (cell_w + layout.gap);
        let y = CONTENT_TOP + row as i32 * (layout.card_h + layout.gap);
        let card = RECT { left: x, top: y, right: x + cell_w, bottom: y + layout.card_h };
        let is_selected = position == selected;
        let is_hovered = hovered == Some(position);
        round_fill(
            hdc,
            card,
            if is_selected { palette.selected } else if is_hovered { palette.surface_alt } else { palette.surface },
            if layout.dense { 9 } else { 11 },
        );

        match entry {
            CatalogRef::Builtin(item_index) => {
                let item = catalog[item_index];
                paint_picker_item(
                    hdc, item.kind, item.content, card, normal_font, symbol_font, mono_font,
                    palette, &mut emoji_draws, &mut emoji_fallbacks,
                );
            }
            CatalogRef::Custom(item_index) => {
                if let Some(item) = custom::get(item_index) {
                    paint_non_emoji_item(hdc, item.kind, &item.content, card, normal_font, symbol_font, mono_font, palette);
                }
            }
        }
    }

    let update_label = match update::status() {
        update::UpdateStatus::Checking => "Checking",
        update::UpdateStatus::Downloading => "Updating",
        update::UpdateStatus::UpToDate => "Up to date",
        update::UpdateStatus::Available(_) => "Update",
        update::UpdateStatus::Failed(_) => "Retry",
        _ => "Update",
    };
    action_button(hdc, small_font, RECT { left: PAD, top: ACTION_Y, right: 104, bottom: ACTION_Y + ACTION_H }, "Manage", palette.surface, palette.text);
    action_button(hdc, small_font, RECT { left: 112, top: ACTION_Y, right: 190, bottom: ACTION_Y + ACTION_H }, "About", palette.surface, palette.text);
    action_button(hdc, small_font, RECT { left: 198, top: ACTION_Y, right: 302, bottom: ACTION_Y + ACTION_H }, update_label, palette.surface, if matches!(update::status(), update::UpdateStatus::Available(_) | update::UpdateStatus::Failed(_)) { palette.event } else { palette.text });
    let theme_label = match settings::get().theme { settings::Theme::Dark => "Light", settings::Theme::Light => "Dark" };
    action_button(hdc, small_font, RECT { left: 310, top: ACTION_Y, right: POPUP_W - PAD, bottom: ACTION_Y + ACTION_H }, theme_label, palette.surface, palette.accent);

    // Match GahYar: both footer fields use the gold accent and are visually
    // centered in their half of the footer instead of hugging the edges.
    let split = POPUP_W / 2;
    let mut version_rect = RECT { left: PAD, top: FOOTER_Y, right: split, bottom: FOOTER_Y + FOOTER_H };
    draw_text(hdc, small_font, &format!("Windows Emoji Picker v{}", env!("CARGO_PKG_VERSION")), &mut version_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS, palette.accent);
    let mut author_rect = RECT { left: split, top: FOOTER_Y, right: POPUP_W - PAD, bottom: FOOTER_Y + FOOTER_H };
    draw_text(hdc, small_font, "Emad Ghasemi - emadghasemi.ir", &mut author_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS, palette.accent);

    let color_ok = renderer::draw_color_emojis(hdc as *mut std::ffi::c_void, POPUP_W, POPUP_H, &emoji_draws);
    if !color_ok {
        for (text, mut rect) in emoji_fallbacks {
            draw_text(hdc, emoji_font, text, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS, palette.text);
        }
    }

    if buffered {
        BitBlt(screen, 0, 0, client.right, client.bottom, mem, 0, 0, SRCCOPY);
        SelectObject(mem, old_bitmap);
        DeleteObject(bitmap as _);
        DeleteDC(mem);
    } else if !mem.is_null() {
        DeleteDC(mem);
    }

    for font in [normal_font, small_font, symbol_font, mono_font, emoji_font] {
        if !font.is_null() { DeleteObject(font as _); }
    }
    EndPaint(hwnd, &ps);
}

unsafe fn paint_picker_item<'a>(
    hdc: HDC,
    kind: ItemKind,
    content: &'a str,
    card: RECT,
    normal_font: HFONT,
    symbol_font: HFONT,
    mono_font: HFONT,
    palette: Palette,
    emoji_draws: &mut Vec<EmojiDraw<'a>>,
    emoji_fallbacks: &mut Vec<(&'a str, RECT)>,
) {
    if kind == ItemKind::Emoji {
        let glyph_rect = RECT { left: card.left + 2, top: card.top + 1, right: card.right - 2, bottom: card.bottom - 1 };
        emoji_draws.push(EmojiDraw { text: content, left: glyph_rect.left, top: glyph_rect.top, right: glyph_rect.right, bottom: glyph_rect.bottom });
        emoji_fallbacks.push((content, glyph_rect));
        return;
    }
    paint_non_emoji_item(hdc, kind, content, card, normal_font, symbol_font, mono_font, palette);
}

unsafe fn paint_non_emoji_item(
    hdc: HDC,
    kind: ItemKind,
    content: &str,
    card: RECT,
    normal_font: HFONT,
    symbol_font: HFONT,
    mono_font: HFONT,
    palette: Palette,
) {
    let mut rect = RECT { left: card.left + 7, top: card.top + 4, right: card.right - 7, bottom: card.bottom - 4 };
    let (font, format) = match kind {
        ItemKind::Symbol => (symbol_font, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS),
        ItemKind::Ascii => (mono_font, DT_CENTER | DT_VCENTER | DT_WORDBREAK | DT_EDITCONTROL),
        ItemKind::Kaomoji => (symbol_font, DT_CENTER | DT_VCENTER | DT_WORDBREAK | DT_EDITCONTROL),
        ItemKind::Snippet => (normal_font, DT_CENTER | DT_VCENTER | DT_WORDBREAK | DT_EDITCONTROL),
        ItemKind::Emoji => (normal_font, DT_CENTER | DT_VCENTER | DT_SINGLELINE),
    };

    // Pass the original text to DrawTextW. Explicit CR/LF characters are kept,
    // so multiline snippets, kaomoji and ASCII art render on their real lines.
    draw_text(hdc, font, content, &mut rect, format, palette.text);
}

unsafe fn action_button(hdc: HDC, font: HFONT, rect: RECT, text: &str, bg: COLORREF, fg: COLORREF) {
    round_fill(hdc, rect, bg, 9);
    let mut text_rect = rect;
    draw_text(hdc, font, text, &mut text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS, fg);
}
