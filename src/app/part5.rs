unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    if hdc.is_null() {
        return;
    }

    let mut client: RECT = zeroed();
    GetClientRect(hwnd, &mut client);

    fill(hdc, &client, rgb(27, 27, 30));

    let normal_face = wide("Segoe UI");
    let emoji_face = wide("Segoe UI Emoji");
    let normal_font = CreateFontW(
        -16,
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        1,
        0,
        0,
        5,
        0,
        normal_face.as_ptr(),
    );
    let small_font = CreateFontW(
        -14,
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        1,
        0,
        0,
        5,
        0,
        normal_face.as_ptr(),
    );
    let content_font = CreateFontW(
        -27,
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        1,
        0,
        0,
        5,
        0,
        emoji_face.as_ptr(),
    );

    SetBkMode(hdc, 1);

    // Search field.
    let search_rect = RECT {
        left: PAD,
        top: SEARCH_Y,
        right: POPUP_W - PAD,
        bottom: SEARCH_Y + SEARCH_H,
    };
    fill(hdc, &search_rect, rgb(42, 42, 46));

    let query = STATE.with(|cell| String::from_utf16_lossy(&cell.borrow().query_utf16));
    let mut text_rect = RECT {
        left: search_rect.left + 14,
        top: search_rect.top,
        right: search_rect.right - 12,
        bottom: search_rect.bottom,
    };
    if query.is_empty() {
        draw_text(
            hdc,
            normal_font,
            "Search emoji, kaomoji, ASCII, text...",
            &mut text_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            rgb(145, 145, 153),
        );
    } else {
        draw_text(
            hdc,
            normal_font,
            &query,
            &mut text_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            rgb(241, 241, 244),
        );
    }

    let (category_index, selected, scroll, hovered, filtered_count, layout, visible_items) =
        STATE.with(|cell| {
            let state = cell.borrow();
            let layout = grid_layout(&state);
            let visible_items = state
                .filtered
                .iter()
                .skip(state.scroll)
                .take(layout.cols * layout.rows)
                .copied()
                .collect::<Vec<_>>();
            (
                state.category_index,
                state.selected,
                state.scroll,
                state.hovered,
                state.filtered.len(),
                layout,
                visible_items,
            )
        });

    // Category tabs.
    let tab_width = (POPUP_W - PAD * 2) / ItemKind::ALL.len() as i32;
    for index in 0..ItemKind::ALL.len() {
        let left = PAD + tab_width * index as i32;
        let right = if index + 1 == ItemKind::ALL.len() {
            POPUP_W - PAD
        } else {
            left + tab_width - 2
        };
        let tab = RECT {
            left,
            top: TABS_Y,
            right,
            bottom: TABS_Y + TABS_H,
        };
        fill(
            hdc,
            &tab,
            if index == category_index {
                rgb(64, 94, 159)
            } else {
                rgb(35, 35, 39)
            },
        );

        let label = match ItemKind::ALL[index] {
            None => "All",
            Some(kind) => kind.label(),
        };
        let mut label_rect = tab;
        draw_text(
            hdc,
            small_font,
            label,
            &mut label_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            if index == category_index {
                rgb(255, 255, 255)
            } else {
                rgb(190, 190, 198)
            },
        );
    }

    // Grid. Emoji cells are intentionally icon-only; names remain searchable
    // metadata and are not drawn under the glyph.
    let cell_w = card_width(layout);
    let catalog = items();
    let mut emoji_draws = Vec::<EmojiDraw<'_>>::new();
    let mut emoji_fallbacks = Vec::<(&str, RECT)>::new();

    for (slot, item_index) in visible_items.iter().copied().enumerate() {
        let position = scroll + slot;
        let item = catalog[item_index];
        let row = slot / layout.cols;
        let col = slot % layout.cols;
        let x = PAD + col as i32 * (cell_w + layout.gap);
        let y = CONTENT_TOP + row as i32 * (layout.card_h + layout.gap);
        let card = RECT {
            left: x,
            top: y,
            right: x + cell_w,
            bottom: y + layout.card_h,
        };

        let is_selected = position == selected;
        let is_hovered = hovered == Some(position);
        let color = if is_selected {
            rgb(65, 78, 108)
        } else if is_hovered {
            rgb(51, 51, 57)
        } else {
            rgb(37, 37, 41)
        };
        fill(hdc, &card, color);

        if item.kind == ItemKind::Emoji {
            let glyph_rect = RECT {
                left: card.left + 2,
                top: card.top + 1,
                right: card.right - 2,
                bottom: card.bottom - 1,
            };
            emoji_draws.push(EmojiDraw {
                text: item.content,
                left: glyph_rect.left,
                top: glyph_rect.top,
                right: glyph_rect.right,
                bottom: glyph_rect.bottom,
            });
            emoji_fallbacks.push((item.content, glyph_rect));
            continue;
        }

        let show_title = !layout.emoji_dense;
        let mut content_rect = if show_title {
            RECT {
                left: card.left + 8,
                top: card.top + 5,
                right: card.right - 8,
                bottom: card.top + 39,
            }
        } else {
            RECT {
                left: card.left + 5,
                top: card.top + 3,
                right: card.right - 5,
                bottom: card.bottom - 3,
            }
        };
        let preview = preview_text(item);
        let is_symbol = item.kind == ItemKind::Symbol;
        draw_text(
            hdc,
            if is_symbol { content_font } else { normal_font },
            &preview,
            &mut content_rect,
            if is_symbol {
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS
            } else {
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS
            },
            rgb(245, 245, 247),
        );

        if show_title {
            let mut title_rect = RECT {
                left: card.left + 8,
                top: card.top + 39,
                right: card.right - 8,
                bottom: card.bottom - 4,
            };
            draw_text(
                hdc,
                small_font,
                item.title,
                &mut title_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                rgb(158, 158, 168),
            );
        }
    }

    let footer_rect = RECT {
        left: PAD,
        top: FOOTER_Y,
        right: POPUP_W - PAD,
        bottom: FOOTER_Y + FOOTER_H,
    };
    let status = if filtered_count == 0 {
        "No results".to_string()
    } else {
        format!("{} results  •  Enter: insert  •  Esc: close", filtered_count)
    };
    let mut footer_text = footer_rect;
    draw_text(
        hdc,
        small_font,
        &status,
        &mut footer_text,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        rgb(125, 125, 134),
    );

    // Direct2D/DirectWrite color-font overlay. If initialization or drawing is
    // unavailable, fall back to the old monochrome GDI path.
    let color_ok = renderer::draw_color_emojis(
        hdc as *mut std::ffi::c_void,
        POPUP_W,
        POPUP_H,
         &emoji_draws,
    );
    if !color_ok {
        for (text, mut rect) in emoji_fallbacks {
            draw_text(
                hdc,
                content_font,
                text,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                rgb(245, 245, 247),
            );
        }
    }

    if !normal_font.is_null() {
        DeleteObject(normal_font as _);
    }
    if !small_font.is_null() {
        DeleteObject(small_font as _);
    }
    if !content_font.is_null() {
        DeleteObject(content_font as _);
    }

    EndPaint(hwnd, &ps);
}

