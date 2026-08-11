#[derive(Clone, Copy)]
struct GridLayout {
    cols: usize,
    rows: usize,
    gap: i32,
    card_h: i32,
    emoji_dense: bool,
}

fn grid_layout(state: &AppState) -> GridLayout {
    let emoji_dense = ItemKind::ALL[state.category_index] == ItemKind::Emoji;
    if emoji_dense {
        GridLayout {
            cols: EMOJI_COLS,
            rows: EMOJI_ROWS,
            gap: EMOJI_GAP,
            card_h: EMOJI_CARD_H,
            emoji_dense: true,
        }
    } else {
        GridLayout {
            cols: STANDARD_COLS,
            rows: STANDARD_ROWS,
            gap: STANDARD_GAP,
            card_h: STANDARD_CARD_H,
            emoji_dense: false,
        }
    }
}

fn hit_test_item(x: i32, y: i32, state: &AppState) -> Option<usize> {
    if y < CONTENT_TOP || y >= ACTION_Y - 8 {
        return None;
    }

    let layout = grid_layout(state);
    let cell_w = card_width(layout);
    let relative_x = x - PAD;
    let relative_y = y - CONTENT_TOP;
    if relative_x < 0 || relative_y < 0 {
        return None;
    }

    let stride_x = cell_w + layout.gap;
    let stride_y = layout.card_h + layout.gap;
    let col = (relative_x / stride_x) as usize;
    let row = (relative_y / stride_y) as usize;
    if col >= layout.cols || row >= layout.rows {
        return None;
    }

    if relative_x % stride_x >= cell_w || relative_y % stride_y >= layout.card_h {
        return None;
    }

    let position = state.scroll + row * layout.cols + col;
    (position < state.filtered.len()).then_some(position)
}

fn rebuild_filter(state: &mut AppState) {
    let query = normalize_search(&String::from_utf16_lossy(&state.query_utf16));
    let category = ItemKind::ALL[state.category_index];

    state.filtered.clear();
    for (index, item) in items().iter().enumerate() {
        if item.kind != category || !matches_query(item.title, item.content, item.keywords, &query) {
            continue;
        }
        state.filtered.push(CatalogRef::Builtin(index));
    }

    custom::with_items(|custom_items| {
        for (index, item) in custom_items.iter().enumerate() {
            if item.kind != category || !matches_query(&item.title, &item.content, &item.keywords, &query) {
                continue;
            }
            state.filtered.push(CatalogRef::Custom(index));
        }
    });
}

fn matches_query(title: &str, content: &str, keywords: &str, query: &str) -> bool {
    if query.is_empty() { return true; }
    normalize_search(title).contains(query)
        || normalize_search(content).contains(query)
        || normalize_search(keywords).contains(query)
}

fn normalize_search(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| match ch {
            'ي' | 'ى' => 'ی',
            'ك' => 'ک',
            '\u{200c}' | '\u{200d}' => ' ',
            other => other,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn ensure_selected_visible(state: &mut AppState, count: usize) {
    let layout = grid_layout(state);
    let visible = layout.cols * layout.rows;
    if state.selected < state.scroll {
        state.scroll = (state.selected / layout.cols) * layout.cols;
    } else if state.selected >= state.scroll + visible {
        let selected_row = state.selected / layout.cols;
        state.scroll = selected_row
            .saturating_sub(layout.rows - 1)
            .saturating_mul(layout.cols);
    }
    state.scroll = state.scroll.min(max_scroll(count, layout));
}

fn max_scroll(count: usize, layout: GridLayout) -> usize {
    let rows = (count + layout.cols - 1) / layout.cols;
    rows.saturating_sub(layout.rows) * layout.cols
}

fn card_width(layout: GridLayout) -> i32 {
    (POPUP_W - PAD * 2 - layout.gap * (layout.cols as i32 - 1)) / layout.cols as i32
}
