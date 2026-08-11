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
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

use crate::{
    custom::{self, CustomItem},
    model::ItemKind,
    native_window::{point_from_lparam, NativeWindowBase},
    theme::Palette,
};

const CLASS_NAME: &str = "WindowsEmojiPicker.CustomManager";
const DEFAULT_W: i32 = 720;
const DEFAULT_H: i32 = 620;
const MIN_W: i32 = 620;
const MIN_H: i32 = 520;
const RADIUS: i32 = 18;

const PAD: i32 = 16;
const HEADER_BOTTOM: i32 = 46;
const LIST_X: i32 = 16;
const LIST_Y: i32 = 54;
const LIST_W: i32 = 190;
const LIST_ROW_H: i32 = 30;
const EDIT_X: i32 = 222;
const EDIT_SUBCLASS_ID: usize = 0x4550;

const BASE: NativeWindowBase = NativeWindowBase::new(DEFAULT_W, DEFAULT_H, RADIUS);

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
            hwnd: null_mut(),
            owner: null_mut(),
            changed_message: 0,
            selected: None,
            list_scroll: 0,
            kind: ItemKind::Kaomoji,
            title_edit: null_mut(),
            content_edit: null_mut(),
            keywords_edit: null_mut(),
            font: null_mut(),
            small_font: null_mut(),
            edit_brush: null_mut(),
        }
    }
}

#[derive(Clone, Copy)]
struct ManagerLayout {
    width: i32,
    height: i32,
    list: RECT,
    tabs: [RECT; 3],
    title_label: RECT,
    title_edit: RECT,
    content_label: RECT,
    content_edit: RECT,
    keywords_label: RECT,
    keywords_edit: RECT,
    buttons: [RECT; 4],
}

impl ManagerLayout {
    fn from_client(width: i32, height: i32) -> Self {
        let width = width.max(MIN_W - 20);
        let height = height.max(MIN_H - 20);
        let right = (width - PAD).max(EDIT_X + 260);

        let tabs_top = 54;
        let tabs_h = 32;
        let tabs_gap = 8;
        let tabs_total_w = right - EDIT_X;
        let tab_w = ((tabs_total_w - tabs_gap * 2) / 3).max(80);
        let tabs = [
            RECT { left: EDIT_X, top: tabs_top, right: EDIT_X + tab_w, bottom: tabs_top + tabs_h },
            RECT { left: EDIT_X + tab_w + tabs_gap, top: tabs_top, right: EDIT_X + tab_w * 2 + tabs_gap, bottom: tabs_top + tabs_h },
            RECT { left: EDIT_X + (tab_w + tabs_gap) * 2, top: tabs_top, right, bottom: tabs_top + tabs_h },
        ];

        let title_label = RECT { left: EDIT_X, top: 94, right, bottom: 114 };
        let title_edit = RECT { left: EDIT_X, top: 116, right, bottom: 148 };

        let buttons_top = height - 52;
        let buttons_bottom = height - 16;
        let button_gap = 8;
        let button_w = ((right - EDIT_X - button_gap * 3) / 4).max(70);
        let buttons = [
            RECT { left: EDIT_X, top: buttons_top, right: EDIT_X + button_w, bottom: buttons_bottom },
            RECT { left: EDIT_X + button_w + button_gap, top: buttons_top, right: EDIT_X + button_w * 2 + button_gap, bottom: buttons_bottom },
            RECT { left: EDIT_X + (button_w + button_gap) * 2, top: buttons_top, right: EDIT_X + button_w * 3 + button_gap * 2, bottom: buttons_bottom },
            RECT { left: EDIT_X + (button_w + button_gap) * 3, top: buttons_top, right, bottom: buttons_bottom },
        ];

        let keywords_edit = RECT {
            left: EDIT_X,
            top: buttons_top - 46,
            right,
            bottom: buttons_top - 14,
        };
        let keywords_label = RECT {
            left: EDIT_X,
            top: keywords_edit.top - 24,
            right,
            bottom: keywords_edit.top - 4,
        };
        let content_label = RECT { left: EDIT_X, top: 158, right, bottom: 178 };
        let content_edit = RECT {
            left: EDIT_X,
            top: 180,
            right,
            bottom: (keywords_label.top - 10).max(300),
        };

        let list = RECT {
            left: LIST_X,
            top: LIST_Y,
            right: LIST_X + LIST_W,
            bottom: (buttons_top - 8).max(LIST_Y + LIST_ROW_H),
        };

        Self {
            width,
            height,
            list,
            tabs,
            title_label,
            title_edit,
            content_label,
            content_edit,
            keywords_label,
            keywords_edit,
            buttons,
        }
    }

    fn visible_rows(self) -> usize {
        ((self.list.bottom - self.list.top) / LIST_ROW_H).max(1) as usize
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

    let (x, y) = BASE.centered_on_owner(owner, 8);
    let title = wide("Custom Items");
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP | WS_THICKFRAME,
        x,
        y,
        DEFAULT_W,
        DEFAULT_H,
        owner,
        null_mut(),
        instance,
        null(),
    );
    if hwnd.is_null() {
        return;
    }

    let font = create_font(-16, 400, "Segoe UI");
    let small_font = create_font(-13, 400, "Segoe UI");
    let palette = Palette::current();
    let edit_brush = CreateSolidBrush(palette.surface_alt);

    let edit_class = wide("EDIT");
    let empty = wide("");
    let title_edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        edit_class.as_ptr(),
        empty.as_ptr(),
        WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32,
        0,
        0,
        10,
        10,
        hwnd,
        2001usize as HMENU,
        instance,
        null(),
    );
    let content_edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        edit_class.as_ptr(),
        empty.as_ptr(),
        WS_CHILD
            | WS_VISIBLE
            | ES_MULTILINE as u32
            | ES_AUTOVSCROLL as u32
            | ES_WANTRETURN as u32
            | WS_VSCROLL,
        0,
        0,
        10,
        10,
        hwnd,
        2002usize as HMENU,
        instance,
        null(),
    );
    let keywords_edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        edit_class.as_ptr(),
        empty.as_ptr(),
        WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32,
        0,
        0,
        10,
        10,
        hwnd,
        2003usize as HMENU,
        instance,
        null(),
    );

    for control in [title_edit, content_edit, keywords_edit] {
        SendMessageW(control, WM_SETFONT, font as usize, 1);
        SetWindowSubclass(control, Some(edit_subclass_proc), EDIT_SUBCLASS_ID, 0);
    }

    STATE.with(|cell| {
        *cell.borrow_mut() = ManagerState {
            hwnd,
            owner,
            changed_message,
            selected: None,
            list_scroll: 0,
            kind: ItemKind::Kaomoji,
            title_edit,
            content_edit,
            keywords_edit,
            font,
            small_font,
            edit_brush,
        };
    });

    layout_controls(hwnd);
    NativeWindowBase::apply_rounding_to_window(hwnd, RADIUS);
    ShowWindow(hwnd, SW_SHOW);
    SetForegroundWindow(hwnd);
    SetFocus(title_edit);
}

pub unsafe fn invalidate() {
    let hwnd = STATE.with(|cell| cell.borrow().hwnd);
    if hwnd.is_null() || IsWindow(hwnd) == 0 {
        return;
    }

    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if !state.edit_brush.is_null() {
            DeleteObject(state.edit_brush as _);
        }
        state.edit_brush = CreateSolidBrush(Palette::current().surface_alt);
    });

    InvalidateRect(hwnd, null(), 0);
    let edits = STATE.with(|cell| {
        let state = cell.borrow();
        [state.title_edit, state.content_edit, state.keywords_edit]
    });
    for edit in edits {
        if !edit.is_null() {
            InvalidateRect(edit, null(), 1);
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            if let Some(base) = NativeWindowBase::from_client(hwnd, RADIUS) {
                if let Some(hit) = base.drag_hit_test(hwnd, lparam, |x, y| {
                    manager_point_is_interactive(hwnd, x, y)
                }) {
                    return hit;
                }
            }
        }
        WM_GETMINMAXINFO => {
            let info = lparam as *mut MINMAXINFO;
            if !info.is_null() {
                (*info).ptMinTrackSize.x = MIN_W;
                (*info).ptMinTrackSize.y = MIN_H;
            }
            return 0;
        }
        WM_SIZE => {
            layout_controls(hwnd);
            clamp_scroll(hwnd);
            NativeWindowBase::apply_rounding_to_window(hwnd, RADIUS);
            InvalidateRect(hwnd, null(), 0);
            return 0;
        }
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            paint(hwnd);
            return 0;
        }
        WM_CTLCOLOREDIT => {
            let hdc = wparam as HDC;
            let palette = Palette::current();
            SetTextColor(hdc, palette.text);
            SetBkColor(hdc, palette.surface_alt);
            return STATE.with(|cell| cell.borrow().edit_brush as LRESULT);
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xffff) as u16 as i16;
            let layout = current_layout(hwnd);
            let changed = STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                let old = state.list_scroll;
                let count = custom::snapshot().len();
                let max_scroll = count.saturating_sub(layout.visible_rows());
                if delta < 0 {
                    state.list_scroll = (state.list_scroll + 1).min(max_scroll);
                } else {
                    state.list_scroll = state.list_scroll.saturating_sub(1);
                }
                old != state.list_scroll
            });
            if changed {
                InvalidateRect(hwnd, null(), 0);
            }
            return 0;
        }
        WM_LBUTTONUP => {
            let (x, y) = point_from_lparam(lparam);
            if handle_click(hwnd, x, y) {
                return 0;
            }
        }
        WM_KEYDOWN if wparam as u16 == VK_ESCAPE => {
            DestroyWindow(hwnd);
            return 0;
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            return 0;
        }
        WM_DESTROY => {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                if !state.font.is_null() {
                    DeleteObject(state.font as _);
                }
                if !state.small_font.is_null() {
                    DeleteObject(state.small_font as _);
                }
                if !state.edit_brush.is_null() {
                    DeleteObject(state.edit_brush as _);
                }
                *state = ManagerState::default();
            });
            return 0;
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    subclass_id: usize,
    _ref_data: usize,
) -> LRESULT {
    if msg == WM_KEYDOWN && wparam as u16 == VK_ESCAPE {
        let parent = GetParent(hwnd);
        if !parent.is_null() {
            PostMessageW(parent, WM_CLOSE, 0, 0);
        }
        return 0;
    }

    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(edit_subclass_proc), subclass_id);
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe fn current_layout(hwnd: HWND) -> ManagerLayout {
    let mut client: RECT = zeroed();
    if GetClientRect(hwnd, &mut client) == 0 {
        return ManagerLayout::from_client(DEFAULT_W, DEFAULT_H);
    }
    ManagerLayout::from_client(client.right - client.left, client.bottom - client.top)
}

unsafe fn layout_controls(hwnd: HWND) {
    let layout = current_layout(hwnd);
    let (title_edit, content_edit, keywords_edit) = STATE.with(|cell| {
        let state = cell.borrow();
        (state.title_edit, state.content_edit, state.keywords_edit)
    });

    for (control, rect) in [
        (title_edit, layout.title_edit),
        (content_edit, layout.content_edit),
        (keywords_edit, layout.keywords_edit),
    ] {
        if control.is_null() {
            continue;
        }
        MoveWindow(
            control,
            rect.left,
            rect.top,
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
            1,
        );
    }
}

unsafe fn clamp_scroll(hwnd: HWND) {
    let visible = current_layout(hwnd).visible_rows();
    let count = custom::snapshot().len();
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.list_scroll = state.list_scroll.min(count.saturating_sub(visible));
    });
}

unsafe fn manager_point_is_interactive(hwnd: HWND, x: i32, y: i32) -> bool {
    let layout = current_layout(hwnd);

    if rect_contains(layout.list, x, y) {
        return true;
    }
    if layout.tabs.into_iter().any(|rect| rect_contains(rect, x, y)) {
        return true;
    }
    if [layout.title_edit, layout.content_edit, layout.keywords_edit]
        .into_iter()
        .any(|rect| rect_contains(rect, x, y))
    {
        return true;
    }
    layout.buttons.into_iter().any(|rect| rect_contains(rect, x, y))
}

unsafe fn handle_click(hwnd: HWND, x: i32, y: i32) -> bool {
    let layout = current_layout(hwnd);

    if rect_contains(layout.list, x, y) {
        let row = ((y - layout.list.top) / LIST_ROW_H) as usize;
        let index = STATE.with(|cell| cell.borrow().list_scroll + row);
        if let Some(item) = custom::snapshot().get(index).cloned() {
            let (title_edit, content_edit, keywords_edit) = STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                state.selected = Some(index);
                state.kind = item.kind;
                (state.title_edit, state.content_edit, state.keywords_edit)
            });
            set_text(title_edit, &item.title);
            set_text(content_edit, &item.content);
            set_text(keywords_edit, &item.keywords);
            InvalidateRect(hwnd, null(), 0);
        }
        return true;
    }

    let kinds = [ItemKind::Kaomoji, ItemKind::Ascii, ItemKind::Snippet];
    for (index, kind) in kinds.into_iter().enumerate() {
        if rect_contains(layout.tabs[index], x, y) {
            STATE.with(|cell| cell.borrow_mut().kind = kind);
            InvalidateRect(hwnd, null(), 0);
            return true;
        }
    }

    if rect_contains(layout.buttons[0], x, y) {
        let (title_edit, content_edit, keywords_edit) = STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            state.selected = None;
            state.kind = ItemKind::Kaomoji;
            (state.title_edit, state.content_edit, state.keywords_edit)
        });
        set_text(title_edit, "");
        set_text(content_edit, "");
        set_text(keywords_edit, "");
        SetFocus(title_edit);
        InvalidateRect(hwnd, null(), 0);
        return true;
    }

    if rect_contains(layout.buttons[1], x, y) {
        save_current(hwnd);
        return true;
    }
    if rect_contains(layout.buttons[2], x, y) {
        delete_current(hwnd);
        return true;
    }
    if rect_contains(layout.buttons[3], x, y) {
        DestroyWindow(hwnd);
        return true;
    }

    false
}

unsafe fn save_current(hwnd: HWND) {
    let (selected, kind, title_edit, content_edit, keywords_edit, owner, message) = STATE.with(|cell| {
        let state = cell.borrow();
        (
            state.selected,
            state.kind,
            state.title_edit,
            state.content_edit,
            state.keywords_edit,
            state.owner,
            state.changed_message,
        )
    });

    let content = get_text(content_edit);
    if content.trim().is_empty() {
        let text = wide("Content cannot be empty.");
        let title = wide("Custom Items");
        MessageBoxW(hwnd, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
        return;
    }

    let index = custom::upsert(
        selected,
        CustomItem {
            kind,
            title: get_text(title_edit),
            content,
            keywords: get_text(keywords_edit),
        },
    );
    STATE.with(|cell| cell.borrow_mut().selected = Some(index));
    PostMessageW(owner, message, 0, 0);
    clamp_scroll(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn delete_current(hwnd: HWND) {
    let (selected, owner, message) = STATE.with(|cell| {
        let state = cell.borrow();
        (state.selected, state.owner, state.changed_message)
    });
    let Some(index) = selected else {
        return;
    };

    let text = wide("Delete this custom item?");
    let title = wide("Delete Item");
    if MessageBoxW(hwnd, text.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONQUESTION) != IDYES {
        return;
    }

    if custom::remove(index) {
        let (title_edit, content_edit, keywords_edit) = STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            state.selected = None;
            (state.title_edit, state.content_edit, state.keywords_edit)
        });
        set_text(title_edit, "");
        set_text(content_edit, "");
        set_text(keywords_edit, "");
        PostMessageW(owner, message, 0, 0);
        clamp_scroll(hwnd);
        InvalidateRect(hwnd, null(), 0);
    }
}

unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    if screen.is_null() {
        return;
    }

    let mut client: RECT = zeroed();
    GetClientRect(hwnd, &mut client);
    let layout = ManagerLayout::from_client(client.right, client.bottom);

    let mem = CreateCompatibleDC(screen);
    let bitmap = CreateCompatibleBitmap(screen, client.right.max(1), client.bottom.max(1));
    let old_bitmap = SelectObject(mem, bitmap as _);
    let palette = Palette::current();
    fill(mem, &client, palette.surface);

    let (selected, scroll, kind, font, small_font) = STATE.with(|cell| {
        let state = cell.borrow();
        (state.selected, state.list_scroll, state.kind, state.font, state.small_font)
    });

    draw_text(
        mem,
        font,
        "Custom Items",
        RECT { left: PAD, top: 10, right: layout.width - PAD, bottom: HEADER_BOTTOM },
        palette.accent,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );

    round_fill(mem, layout.list, palette.background, 12);

    let items = custom::snapshot();
    for (slot, item) in items.iter().skip(scroll).take(layout.visible_rows()).enumerate() {
        let index = scroll + slot;
        let top = layout.list.top + slot as i32 * LIST_ROW_H;
        let rect = RECT {
            left: layout.list.left + 4,
            top: top + 2,
            right: layout.list.right - 4,
            bottom: top + LIST_ROW_H - 2,
        };
        if selected == Some(index) {
            round_fill(mem, rect, palette.selected, 7);
        }
        let title = if item.title.trim().is_empty() {
            item.content.lines().next().unwrap_or("Custom")
        } else {
            &item.title
        };
        draw_text(
            mem,
            small_font,
            title,
            RECT { left: rect.left + 7, top: rect.top, right: rect.right - 7, bottom: rect.bottom },
            palette.text,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
    }

    let kinds = [ItemKind::Kaomoji, ItemKind::Ascii, ItemKind::Snippet];
    for (index, item_kind) in kinds.into_iter().enumerate() {
        let rect = layout.tabs[index];
        round_fill(
            mem,
            rect,
            if kind == item_kind { palette.accent } else { palette.surface_alt },
            10,
        );
        draw_text(
            mem,
            small_font,
            item_kind.label(),
            rect,
            if kind == item_kind { palette.accent_text } else { palette.text },
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }

    draw_text(mem, small_font, "Title", layout.title_label, palette.muted, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    draw_text(mem, small_font, "Content", layout.content_label, palette.muted, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
    draw_text(mem, small_font, "Search keywords", layout.keywords_label, palette.muted, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

    button(mem, small_font, layout.buttons[0], "New", palette.surface_alt, palette.text);
    button(mem, small_font, layout.buttons[1], "Save", palette.accent, palette.accent_text);
    button(mem, small_font, layout.buttons[2], "Delete", palette.surface_alt, palette.danger);
    button(mem, small_font, layout.buttons[3], "Close", palette.surface_alt, palette.text);

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
    if brush.is_null() {
        return;
    }
    FillRect(hdc, rect, brush);
    DeleteObject(brush as _);
}

unsafe fn round_fill(hdc: HDC, rect: RECT, color: COLORREF, radius: i32) {
    let brush = CreateSolidBrush(color);
    if brush.is_null() {
        return;
    }
    let old_brush = SelectObject(hdc, brush as _);
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN) as _);
    RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, radius, radius);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    DeleteObject(brush as _);
}

unsafe fn draw_text(hdc: HDC, font: HFONT, text: &str, mut rect: RECT, color: COLORREF, format: u32) {
    if font.is_null() {
        return;
    }
    let old = SelectObject(hdc, font as _);
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, color);
    let mut value: Vec<u16> = text.encode_utf16().collect();
    DrawTextW(hdc, value.as_mut_ptr(), value.len() as i32, &mut rect, format);
    SelectObject(hdc, old);
}

unsafe fn create_font(height: i32, weight: i32, family: &str) -> HFONT {
    let family = wide(family);
    CreateFontW(
        height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        DEFAULT_PITCH as u32 | FF_DONTCARE as u32,
        family.as_ptr(),
    )
}

unsafe fn set_text(hwnd: HWND, value: &str) {
    if hwnd.is_null() {
        return;
    }
    let value = wide(value);
    SetWindowTextW(hwnd, value.as_ptr());
}

unsafe fn get_text(hwnd: HWND) -> String {
    if hwnd.is_null() {
        return String::new();
    }
    let length = GetWindowTextLengthW(hwnd).max(0) as usize;
    let mut buffer = vec![0u16; length + 1];
    let read = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
    String::from_utf16_lossy(&buffer[..read])
}

fn rect_contains(rect: RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
