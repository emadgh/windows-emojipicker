use std::{
    cell::RefCell,
    mem::zeroed,
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE},
        WindowsAndMessaging::*,
    },
};

use crate::{
    custom::{self, CustomItem},
    model::ItemKind,
    theme::Palette,
};

const CLASS_NAME: &str = "WindowsEmojiPicker.CustomManager";
const W: i32 = 620;
const H: i32 = 460;
const LIST_X: i32 = 16;
const LIST_Y: i32 = 54;
const LIST_W: i32 = 194;
const LIST_H: i32 = 330;
const LIST_ROW_H: i32 = 44;

thread_local! {
    static STATE: RefCell<ManagerState> = RefCell::new(ManagerState::default());
}

struct ManagerState {
    hwnd: HWND,
    owner: HWND,
    changed_message: u32,
    selected: Option<usize>,
    list_scroll: usize,
    kind: ItemKind,
    title_edit: HWND,
    content_edit: HWND,
    keywords_edit: HWND,
    font: HFONT,
    small_font: HFONT,
    edit_brush: HBRUSH,
}

impl Default for ManagerState {
    fn default() -> Self {
        Self {
            hwnd: null_mut(), owner: null_mut(), changed_message: 0,
            selected: None, list_scroll: 0, kind: ItemKind::Kaomoji,
            title_edit: null_mut(), content_edit: null_mut(), keywords_edit: null_mut(),
            font: null_mut(), small_font: null_mut(), edit_brush: null_mut(),
        }
    }
}

pub unsafe fn show(owner: HWND, changed_message: u32) {
    let existing = STATE.with(|cell| cell.borrow().hwnd);
    if !existing.is_null() && IsWindow(existing) != 0 {
        ShowWindow(existing, SW_SHOW);
        SetForegroundWindow(existing);
        return;
    }

    let instance = GetModuleHandleW(null());
    let class_name = wide(CLASS_NAME);
    let cursor = LoadCursorW(null_mut(), IDC_ARROW);
    let mut wc: WNDCLASSW = zeroed();
    wc.lpfnWndProc = Some(wnd_proc);
    wc.hInstance = instance;
    wc.hCursor = cursor;
    wc.hbrBackground = null_mut();
    wc.lpszClassName = class_name.as_ptr();
    RegisterClassW(&wc);

    let mut owner_rect: RECT = zeroed();
    GetWindowRect(owner, &mut owner_rect);
    let x = owner_rect.left + ((owner_rect.right - owner_rect.left) - W) / 2;
    let y = owner_rect.top + ((owner_rect.bottom - owner_rect.top) - H) / 2;
    let title = wide("Custom Items");

    // Fully custom chrome: no WS_CAPTION or WS_SYSMENU, so the dialog matches
    // the picker/GahYar visual language instead of showing Windows title-bar UI.
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class_name.as_ptr(), title.as_ptr(),
        WS_POPUP,
        x.max(0), y.max(0), W, H,
        owner, null_mut(), instance, null(),
    );
    if hwnd.is_null() { return; }

    let font = create_font(-16, 400, "Segoe UI");
    let small_font = create_font(-13, 400, "Segoe UI");
    let palette = Palette::current();
    let edit_brush = CreateSolidBrush(palette.surface_alt);

    let edit_class = wide("EDIT");
    let empty = wide("");
    let title_edit = CreateWindowExW(
        WS_EX_CLIENTEDGE, edit_class.as_ptr(), empty.as_ptr(),
        WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32,
        226, 118, 378, 30, hwnd, 2001usize as HMENU, instance, null(),
    );
    let content_edit = CreateWindowExW(
        WS_EX_CLIENTEDGE, edit_class.as_ptr(), empty.as_ptr(),
        WS_CHILD | WS_VISIBLE | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32 | WS_VSCROLL,
        226, 184, 378, 116, hwnd, 2002usize as HMENU, instance, null(),
    );
    let keywords_edit = CreateWindowExW(
        WS_EX_CLIENTEDGE, edit_class.as_ptr(), empty.as_ptr(),
        WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32,
        226, 334, 378, 30, hwnd, 2003usize as HMENU, instance, null(),
    );
    for control in [title_edit, content_edit, keywords_edit] {
        SendMessageW(control, WM_SETFONT, font as usize, 1);
    }

    STATE.with(|cell| {
        *cell.borrow_mut() = ManagerState {
            hwnd, owner, changed_message, selected: None, list_scroll: 0,
            kind: ItemKind::Kaomoji, title_edit, content_edit, keywords_edit,
            font, small_font, edit_brush,
        };
    });

    let region = CreateRoundRectRgn(0, 0, W + 1, H + 1, 18, 18);
    SetWindowRgn(hwnd, region, 1);
    ShowWindow(hwnd, SW_SHOW);
    SetForegroundWindow(hwnd);
    SetFocus(title_edit);
}

pub unsafe fn invalidate() {
    let hwnd = STATE.with(|cell| cell.borrow().hwnd);
    if !hwnd.is_null() && IsWindow(hwnd) != 0 {
        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            if !state.edit_brush.is_null() { DeleteObject(state.edit_brush as _); }
            state.edit_brush = CreateSolidBrush(Palette::current().surface_alt);
        });
        InvalidateRect(hwnd, null(), 0);
        for edit in STATE.with(|cell| {
            let state = cell.borrow();
            [state.title_edit, state.content_edit, state.keywords_edit]
        }) {
            InvalidateRect(edit, null(), 1);
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => { paint(hwnd); return 0; }
        WM_CTLCOLOREDIT => {
            let hdc = wparam as HDC;
            let palette = Palette::current();
            SetTextColor(hdc, palette.text);
            SetBkColor(hdc, palette.surface_alt);
            return STATE.with(|cell| cell.borrow().edit_brush as LRESULT);
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xffff) as u16 as i16;
            let changed = STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                let old = state.list_scroll;
                let count = custom::snapshot().len();
                let visible = (LIST_H / LIST_ROW_H) as usize;
                let max_scroll = count.saturating_sub(visible);
                if delta < 0 { state.list_scroll = (state.list_scroll + 1).min(max_scroll); }
                else { state.list_scroll = state.list_scroll.saturating_sub(1); }
                old != state.list_scroll
            });
            if changed { InvalidateRect(hwnd, null(), 0); }
            return 0;
        }
        WM_LBUTTONUP => {
            let (x, y) = point_from_lparam(lparam);
            if handle_click(hwnd, x, y) { return 0; }
        }
        WM_KEYDOWN if wparam as u16 == VK_ESCAPE => { DestroyWindow(hwnd); return 0; }
        WM_CLOSE => { DestroyWindow(hwnd); return 0; }
        WM_DESTROY => {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                if !state.font.is_null() { DeleteObject(state.font as _); }
                if !state.small_font.is_null() { DeleteObject(state.small_font as _); }
                if !state.edit_brush.is_null() { DeleteObject(state.edit_brush as _); }
                *state = ManagerState::default();
            });
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn handle_click(hwnd: HWND, x: i32, y: i32) -> bool {
    if (LIST_X..LIST_X + LIST_W).contains(&x) && (LIST_Y..LIST_Y + LIST_H).contains(&y) {
        let row = ((y - LIST_Y) / LIST_ROW_H) as usize;
        let index = STATE.with(|cell| cell.borrow().list_scroll + row);
        if let Some(item) = custom::snapshot().get(index).cloned() {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                state.selected = Some(index);
                state.kind = item.kind;
                set_text(state.title_edit, &item.title);
                set_text(state.content_edit, &item.content);
                set_text(state.keywords_edit, &item.keywords);
            });
            InvalidateRect(hwnd, null(), 0);
        }
        return true;
    }

    let kinds = [ItemKind::Kaomoji, ItemKind::Ascii, ItemKind::Snippet];
    for (index, kind) in kinds.into_iter().enumerate() {
        let left = 226 + index as i32 * 124;
        if (left..left + 116).contains(&x) && (54..86).contains(&y) {
            STATE.with(|cell| cell.borrow_mut().kind = kind);
            InvalidateRect(hwnd, null(), 0);
            return true;
        }
    }

    if (226..306).contains(&x) && (398..434).contains(&y) {
        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            state.selected = None;
            state.kind = ItemKind::Kaomoji;
            set_text(state.title_edit, "");
            set_text(state.content_edit, "");
            set_text(state.keywords_edit, "");
            SetFocus(state.title_edit);
        });
        InvalidateRect(hwnd, null(), 0);
        return true;
    }
    if (314..402).contains(&x) && (398..434).contains(&y) {
        save_current(hwnd);
        return true;
    }
    if (410..502).contains(&x) && (398..434).contains(&y) {
        delete_current(hwnd);
        return true;
    }
    if (510..604).contains(&x) && (398..434).contains(&y) {
        DestroyWindow(hwnd);
        return true;
    }
    false
}

unsafe fn save_current(hwnd: HWND) {
    let (selected, kind, title_edit, content_edit, keywords_edit, owner, message) = STATE.with(|cell| {
        let state = cell.borrow();
        (state.selected, state.kind, state.title_edit, state.content_edit, state.keywords_edit, state.owner, state.changed_message)
    });
    let content = get_text(content_edit);
    if content.trim().is_empty() {
        let text = wide("Content cannot be empty.");
        let title = wide("Custom Items");
        MessageBoxW(hwnd, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
        return;
    }
    let index = custom::upsert(selected, CustomItem {
        kind,
        title: get_text(title_edit),
        content,
        keywords: get_text(keywords_edit),
    });
    STATE.with(|cell| cell.borrow_mut().selected = Some(index));
    PostMessageW(owner, message, 0, 0);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn delete_current(hwnd: HWND) {
    let (selected, owner, message) = STATE.with(|cell| {
        let state = cell.borrow();
        (state.selected, state.owner, state.changed_message)
    });
    let Some(index) = selected else { return; };
    let text = wide("Delete this custom item?");
    let title = wide("Delete Item");
    if MessageBoxW(hwnd, text.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONQUESTION) != IDYES {
        return;
    }
    if custom::remove(index) {
        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            state.selected = None;
            set_text(state.title_edit, "");
            set_text(state.content_edit, "");
            set_text(state.keywords_edit, "");
        });
        PostMessageW(owner, message, 0, 0);
        InvalidateRect(hwnd, null(), 0);
    }
}

unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    if screen.is_null() { return; }
    let mut client: RECT = zeroed();
    GetClientRect(hwnd, &mut client);
    let mem = CreateCompatibleDC(screen);
    let bitmap = CreateCompatibleBitmap(screen, client.right, client.bottom);
    let old_bitmap = SelectObject(mem, bitmap as _);
    let palette = Palette::current();
    fill(mem, &client, palette.surface);

    let (selected, scroll, kind, font, small_font) = STATE.with(|cell| {
        let state = cell.borrow();
        (state.selected, state.list_scroll, state.kind, state.font, state.small_font)
    });
    draw_text(mem, font, "Custom Items", RECT { left: 18, top: 12, right: 602, bottom: 44 }, palette.accent, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    round_fill(mem, RECT { left: LIST_X, top: LIST_Y, right: LIST_X + LIST_W, bottom: LIST_Y + LIST_H }, palette.background, 12);

    let items = custom::snapshot();
    for (slot, item) in items.iter().skip(scroll).take((LIST_H / LIST_ROW_H) as usize).enumerate() {
        let index = scroll + slot;
        let top = LIST_Y + slot as i32 * LIST_ROW_H;
        let rect = RECT { left: LIST_X + 5, top: top + 4, right: LIST_X + LIST_W - 5, bottom: top + LIST_ROW_H - 4 };
        if selected == Some(index) { round_fill(mem, rect, palette.selected, 9); }
        let title = if item.title.trim().is_empty() { item.content.lines().next().unwrap_or("Custom") } else { &item.title };
        draw_text(mem, small_font, title, RECT { left: rect.left + 8, top: rect.top, right: rect.right - 8, bottom: rect.bottom }, palette.text, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
    }

    let kinds = [ItemKind::Kaomoji, ItemKind::Ascii, ItemKind::Snippet];
    for (index, item_kind) in kinds.into_iter().enumerate() {
        let left = 226 + index as i32 * 124;
        let rect = RECT { left, top: 54, right: left + 116, bottom: 86 };
        round_fill(mem, rect, if kind == item_kind { palette.accent } else { palette.surface_alt }, 10);
        draw_text(mem, small_font, item_kind.label(), rect, if kind == item_kind { palette.accent_text } else { palette.text }, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }

    draw_text(mem, small_font, "Title", RECT { left: 226, top: 92, right: 604, bottom: 114 }, palette.muted, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    draw_text(mem, small_font, "Content", RECT { left: 226, top: 158, right: 604, bottom: 180 }, palette.muted, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    draw_text(mem, small_font, "Search keywords", RECT { left: 226, top: 308, right: 604, bottom: 330 }, palette.muted, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

    button(mem, small_font, RECT { left: 226, top: 398, right: 306, bottom: 434 }, "New", palette.surface_alt, palette.text);
    button(mem, small_font, RECT { left: 314, top: 398, right: 402, bottom: 434 }, "Save", palette.accent, palette.accent_text);
    button(mem, small_font, RECT { left: 410, top: 398, right: 502, bottom: 434 }, "Delete", palette.surface_alt, palette.danger);
    button(mem, small_font, RECT { left: 510, top: 398, right: 604, bottom: 434 }, "Close", palette.surface_alt, palette.text);

    BitBlt(screen, 0, 0, client.right, client.bottom, mem, 0, 0, SRCCOPY);
    SelectObject(mem, old_bitmap);
    DeleteObject(bitmap as _);
    DeleteDC(mem);
    EndPaint(hwnd, &ps);
}

unsafe fn button(hdc: HDC, font: HFONT, rect: RECT, text: &str, bg: COLORREF, fg: COLORREF) {
    round_fill(hdc, rect, bg, 10);
    draw_text(hdc, font, text, rect, fg, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
}

unsafe fn fill(hdc: HDC, rect: &RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    FillRect(hdc, rect, brush);
    DeleteObject(brush as _);
}

unsafe fn round_fill(hdc: HDC, rect: RECT, color: COLORREF, radius: i32) {
    let brush = CreateSolidBrush(color);
    let old_brush = SelectObject(hdc, brush as _);
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN) as _);
    RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, radius, radius);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    DeleteObject(brush as _);
}

unsafe fn draw_text(hdc: HDC, font: HFONT, text: &str, mut rect: RECT, color: COLORREF, format: u32) {
    if font.is_null() { return; }
    let old = SelectObject(hdc, font as _);
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, color);
    let mut value: Vec<u16> = text.encode_utf16().collect();
    DrawTextW(hdc, value.as_mut_ptr(), value.len() as i32, &mut rect, format);
    SelectObject(hdc, old);
}

unsafe fn create_font(height: i32, weight: i32, family: &str) -> HFONT {
    let family = wide(family);
    CreateFontW(height, 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32, DEFAULT_PITCH as u32 | FF_DONTCARE as u32, family.as_ptr())
}

unsafe fn set_text(hwnd: HWND, value: &str) {
    let value = wide(value);
    SetWindowTextW(hwnd, value.as_ptr());
}

unsafe fn get_text(hwnd: HWND) -> String {
    let length = GetWindowTextLengthW(hwnd).max(0) as usize;
    let mut buffer = vec![0u16; length + 1];
    let read = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
    String::from_utf16_lossy(&buffer[..read])
}

fn point_from_lparam(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam as u32 & 0xffff) as u16 as i16 as i32;
    let y = ((lparam as u32 >> 16) & 0xffff) as u16 as i16 as i32;
    (x, y)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
