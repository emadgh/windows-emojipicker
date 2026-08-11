#[derive(Clone, Copy)]
struct GridLayout {
    cols: usize,
    rows: usize,
    gap: i32,
    card_h: i32,
    emoji_dense: bool,
}

fn grid_layout(state: &AppState) -> GridLayout {
    let emoji_dense = ItemKind::ALL[state.category_index] == Some(ItemKind::Emoji);
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
    if y < CONTENT_TOP || y >= FOOTER_Y - 8 {
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
    let query = String::from_utf16_lossy(&state.query_utf16)
        .trim()
        .to_lowercase();
    let category = ItemKind::ALL[state.category_index];

    state.filtered.clear();
    state.filtered.extend(
        items()
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
            .map(|(index, _)| index),
    );
}

fn ensure_selected_visible(state: &mut AppState, count: usize) {
    let layout = grid_layout(state);
    let visible = layout.cols * layout.rows;
    if state.selected < state.scroll {
        state.scroll = (state.selected / layout.cols) * layout.cols;
    } else if state.selected >= state.scroll + visible {
        let selected_row = state.selected / layout.cols;
        state.scroll = selected_row
            .saturating_sub( layout.rows - 1)
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

