use std::{
    cell::RefCell,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};

use crate::{
    model::ItemKind,
    native_window::NativeWindowBase,
    renderer,
    theme::Palette,
};

const CLASS_NAME: &str = "WindowsEmojiPicker.Preview";
const RADIUS: i32 = 18;
const PAD: i32 = 18;
const MIN_W: i32 = 96;
const MIN_H: i32 = 72;
const WINDOW_GAP: i32 = 10;

thread_local! {
    static STATE: RefCell<PreviewState> = RefCell::new(PreviewState::default());
}

struct PreviewState {
    hwnd: HWND,
    owner: HWND,
    kind: Option<ItemKind>,
    title: String,
    content: String,
    font_px: i32,
    width: i32,
    height: i32,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            hwnd: null_mut(),
            owner: null_mut(),
            kind: None,
            title: String::new(),
            content: String::new(),
            font_px: 16,
            width: MIN_W,
            height: MIN_H,
        }
    }
}

#[derive(Clone, Copy)]
struct PreviewLayout {
    width: i32,
    height: i32,
    font_px: i32,
}

pub unsafe fn shutdown() {
    let hwnd = STATE.with(|cell| cell.borrow().hwnd);
    if !hwnd.is_null() && IsWindow(hwnd) != 0 {
        DestroyWindow(hwnd);
    }
}

pub unsafe fn hide() {
    let hwnd = STATE.with(|cell| cell.borrow().hwnd);
    if !hwnd.is_null() && IsWindow(hwnd) != 0 {
        ShowWindow(hwnd, SW_HIDE);
    }
}

pub unsafe fn invalidate() {
    let hwnd = STATE.with(|cell| cell.borrow().hwnd);
    if !hwnd.is_null() && IsWindow(hwnd) != 0 && IsWindowVisible(hwnd) != 0 {
        InvalidateRect(hwnd, null(), 0);
    }
}

pub unsafe fn is_visible() -> bool {
    let hwnd = STATE.with(|cell| cell.borrow().hwnd);
    !hwnd.is_null() && IsWindow(hwnd) != 0 && IsWindowVisible(hwnd) != 0
}

unsafe fn ensure_window(owner: HWND) -> HWND {
    let existing = STATE.with(|cell| cell.borrow().hwnd);
    if !existing.is_null() && IsWindow(existing) != 0 {
        return existing;
    }

    let instance = GetModuleHandleW(null());
    if instance.is_null() {
        return null_mut();
    }

    let class_name = wide(CLASS_NAME);
    let mut wc: WNDCLASSW = zeroed();
    wc.lpfnWndProc = Some(wnd_proc);
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    wc.hbrBackground = null_mut();
    wc.lpszClassName = class_name.as_ptr();
    RegisterClassW(&wc);

    let title = wide("Preview");
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP,
        0,
        0,
        MIN_W,
        MIN_H,
        owner,
        null_mut(),
        instance,
        null(),
    );
    if hwnd.is_null() {
        return null_mut();
    }

    STATE.with(|cell| cell.borrow_mut().hwnd = hwnd);
    hwnd
}

pub unsafe fn show(owner: HWND, kind: ItemKind, title: &str, content: &str) {
    let hwnd = ensure_window(owner);
    if hwnd.is_null() {
        return;
    }

    let layout = measure_layout(owner, kind, content);
    let (x, y) = position_next_to_owner(owner, layout.width, layout.height);
    apply_content(hwnd, owner, kind, title, content, layout);

    let title_w = wide(if title.trim().is_empty() { "Preview" } else { title });
    SetWindowTextW(hwnd, title_w.as_ptr());
    NativeWindowBase::new(layout.width, layout.height, RADIUS).apply_rounding(hwnd);
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        x,
        y,
        layout.width,
        layout.height,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    InvalidateRect(hwnd, null(), 0);
}

pub unsafe fn update(owner: HWND, kind: ItemKind, title: &str, content: &str) {
    let hwnd = STATE.with(|cell| cell.borrow().hwnd);
    if hwnd.is_null() || IsWindow(hwnd) == 0 || IsWindowVisible(hwnd) == 0 {
        return;
    }

    let unchanged = STATE.with(|cell| {
        let state = cell.borrow();
        state.kind == Some(kind) && state.title == title && state.content == content
    });
    if unchanged {
        return;
    }

    let layout = measure_layout(owner, kind, content);
    let mut rect: RECT = zeroed();
    GetWindowRect(hwnd, &mut rect);
    let (x, y) = clamp_existing_position(owner, rect.left, rect.top, layout.width, layout.height);
    apply_content(hwnd, owner, kind, title, content, layout);

    let title_w = wide(if title.trim().is_empty() { "Preview" } else { title });
    SetWindowTextW(hwnd, title_w.as_ptr());
    NativeWindowBase::new(layout.width, layout.height, RADIUS).apply_rounding(hwnd);
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        x,
        y,
        layout.width,
        layout.height,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    InvalidateRect(hwnd, null(), 0);
}

fn apply_content(
    hwnd: HWND,
    owner: HWND,
    kind: ItemKind,
    title: &str,
    content: &str,
    layout: PreviewLayout,
) {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.hwnd = hwnd;
        state.owner = owner;
        state.kind = Some(kind);
        state.title.clear();
        state.title.push_str(title);
        state.content.clear();
        state.content.push_str(content);
        state.font_px = layout.font_px;
        state.width = layout.width;
        state.height = layout.height;
    });
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
        WM_NCHITTEST => {
            if let Some(base) = NativeWindowBase::from_client(hwnd, RADIUS) {
                if let Some(hit) = base.drag_hit_test(hwnd, lparam, |_x, _y| false) {
                    return hit;
                }
            }
        }
        WM_RBUTTONUP => {
            ShowWindow(hwnd, SW_HIDE);
            return 0;
        }
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            paint(hwnd);
            return 0;
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            return 0;
        }
        WM_DESTROY => {
            STATE.with(|cell| *cell.borrow_mut() = PreviewState::default());
            return 0;
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn measure_layout(owner: HWND, kind: ItemKind, content: &str) -> PreviewLayout {
    let work = work_area(owner);
    let max_w = (work.right - work.left - 24).max(MIN_W);
    let max_h = (work.bottom - work.top - 24).max(MIN_H);

    match kind {
        ItemKind::Emoji => PreviewLayout {
            width: 132.min(max_w),
            height: 132.min(max_h),
            font_px: 72,
        },
        ItemKind::Symbol => fit_exact(kind, content, 52, 22, max_w, max_h),
        ItemKind::Kaomoji => fit_exact(kind, content, 26, 12, max_w, max_h),
        ItemKind::Ascii => fit_exact(kind, content, 18, 7, max_w, max_h),
        ItemKind::Snippet => fit_wrapped(kind, content, 19, 11, max_w, max_h),
    }
}

unsafe fn fit_exact(
    kind: ItemKind,
    content: &str,
    start_px: i32,
    min_px: i32,
    max_w: i32,
    max_h: i32,
) -> PreviewLayout {
    let mut px = start_px;
    loop {
        let (content_w, content_h) = measure_exact(kind, content, px);
        let wanted_w = (content_w + PAD * 2).max(MIN_W);
        let wanted_h = (content_h + PAD * 2).max(MIN_H);
        if (wanted_w <= max_w && wanted_h <= max_h) || px <= min_px {
            return PreviewLayout {
                width: wanted_w.min(max_w),
                height: wanted_h.min(max_h),
                font_px: px,
            };
        }
        px -= 1;
    }
}

unsafe fn fit_wrapped(
    kind: ItemKind,
    content: &str,
    start_px: i32,
    min_px: i32,
    max_w: i32,
    max_h: i32,
) -> PreviewLayout {
    let mut px = start_px;
    loop {
        let (natural_w, _) = measure_exact(kind, content, px);
        let wrap_w = natural_w
            .max(180)
            .min(640)
            .min((max_w - PAD * 2).max(80));
        let content_h = measure_wrapped(kind, content, px, wrap_w);
        let wanted_w = (wrap_w + PAD * 2).max(MIN_W);
        let wanted_h = (content_h + PAD * 2).max(MIN_H);
        if (wanted_w <= max_w && wanted_h <= max_h) || px <= min_px {
            return PreviewLayout {
                width: wanted_w.min(max_w),
                height: wanted_h.min(max_h),
                font_px: px,
            };
        }
        px -= 1;
    }
}

unsafe fn measure_exact(kind: ItemKind, content: &str, font_px: i32) -> (i32, i32) {
    let dc = CreateCompatibleDC(null_mut());
    if dc.is_null() {
        return approximate_measure(content, font_px);
    }

    let font = create_preview_font(kind, font_px);
    let old = if font.is_null() { null_mut() } else { SelectObject(dc, font as _) };
    let normalized = normalize_content(content);
    let lines: Vec<&str> = normalized.split('\n').collect();

    let mut line_height = font_px + 5;
    let sample: Vec<u16> = "Mg".encode_utf16().collect();
    let mut sample_size: SIZE = zeroed();
    if GetTextExtentPoint32W(dc, sample.as_ptr(), sample.len() as i32, &mut sample_size) != 0 {
        line_height = sample_size.cy.max(1);
    }

    let mut max_width = 0;
    for line in &lines {
        if line.is_empty() {
            continue;
        }
        let text: Vec<u16> = line.encode_utf16().collect();
        let mut size: SIZE = zeroed();
        if GetTextExtentPoint32W(dc, text.as_ptr(), text.len() as i32, &mut size) != 0 {
            max_width = max_width.max(size.cx);
        }
    }

    if !old.is_null() {
        SelectObject(dc, old);
    }
    if !font.is_null() {
        DeleteObject(font as _);
    }
    DeleteDC(dc);

    (max_width.max(1), line_height.saturating_mul(lines.len().max(1) as i32))
}

unsafe fn measure_wrapped(kind: ItemKind, content: &str, font_px: i32, wrap_w: i32) -> i32 {
    let dc = CreateCompatibleDC(null_mut());
    if dc.is_null() {
        return approximate_measure(content, font_px).1;
    }
    let font = create_preview_font(kind, font_px);
    let old = if font.is_null() { null_mut() } else { SelectObject(dc, font as _) };
    let normalized = normalize_content(content);
    let mut text: Vec<u16> = normalized.encode_utf16().collect();
    let mut rect = RECT { left: 0, top: 0, right: wrap_w.max(1), bottom: 0 };
    DrawTextW(
        dc,
        text.as_mut_ptr(),
        text.len() as i32,
        &mut rect,
        DT_LEFT | DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX | DT_CALCRECT,
    );

    if !old.is_null() {
        SelectObject(dc, old);
    }
    if !font.is_null() {
        DeleteObject(font as _);
    }
    DeleteDC(dc);
    (rect.bottom - rect.top).max(font_px + 4)
}

fn approximate_measure(content: &str, font_px: i32) -> (i32, i32) {
    let normalized = normalize_content(content);
    let lines: Vec<&str> = normalized.split('\n').collect();
    let columns = lines.iter().map(|line| line.chars().count()).max().unwrap_or(1) as i32;
    (
        (columns * font_px / 2).max(1),
        (lines.len().max(1) as i32 * (font_px + 5)).max(1),
    )
}

unsafe fn position_next_to_owner(owner: HWND, width: i32, height: i32) -> (i32, i32) {
    let work = work_area(owner);
    let mut owner_rect: RECT = zeroed();
    if owner.is_null() || GetWindowRect(owner, &mut owner_rect) == 0 {
        return (
            work.left + ((work.right - work.left - width) / 2).max(0),
            work.top + ((work.bottom - work.top - height) / 2).max(0),
        );
    }

    let right_x = owner_rect.right + WINDOW_GAP;
    let left_x = owner_rect.left - WINDOW_GAP - width;
    let mut x = if right_x + width <= work.right {
        right_x
    } else if left_x >= work.left {
        left_x
    } else {
        (owner_rect.left + owner_rect.right - width) / 2
    };
    let mut y = owner_rect.top + ((owner_rect.bottom - owner_rect.top) - height) / 2;
    clamp_xy(&mut x, &mut y, width, height, work);
    (x, y)
}

unsafe fn clamp_existing_position(
    owner: HWND,
    mut x: i32,
    mut y: i32,
    width: i32,
    height: i32,
) -> (i32, i32) {
    let work = work_area(owner);
    clamp_xy(&mut x, &mut y, width, height, work);
    (x, y)
}

fn clamp_xy(x: &mut i32, y: &mut i32, width: i32, height: i32, work: RECT) {
    let max_x = (work.right - width).max(work.left);
    let max_y = (work.bottom - height).max(work.top);
    *x = (*x).clamp(work.left, max_x);
    *y = (*y).clamp(work.top, max_y);
}

unsafe fn work_area(owner: HWND) -> RECT {
    let monitor = if !owner.is_null() {
        MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST)
    } else {
        MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY)
    };
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if !monitor.is_null() && GetMonitorInfoW(monitor, &mut info) != 0 {
        info.rcWork
    } else {
        RECT {
            left: 0,
            top: 0,
            right: GetSystemMetrics(SM_CXSCREEN),
            bottom: GetSystemMetrics(SM_CYSCREEN),
        }
    }
}

unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    if screen.is_null() {
        return;
    }

    let (kind, content, font_px, width, height) = STATE.with(|cell| {
        let state = cell.borrow();
        (
            state.kind,
            state.content.clone(),
            state.font_px,
            state.width,
            state.height,
        )
    });
    let Some(kind) = kind else {
        EndPaint(hwnd, &ps);
        return;
    };

    let mem = CreateCompatibleDC(screen);
    let bitmap = if !mem.is_null() {
        CreateCompatibleBitmap(screen, width.max(1), height.max(1))
    } else {
        null_mut()
    };
    let buffered = !mem.is_null() && !bitmap.is_null();
    let old_bitmap = if buffered { SelectObject(mem, bitmap as _) } else { null_mut() };
    let hdc = if buffered { mem } else { screen };

    let palette = Palette::current();
    fill(hdc, RECT { left: 0, top: 0, right: width, bottom: height }, palette.surface);

    let inner = RECT {
        left: PAD,
        top: PAD,
        right: width - PAD,
        bottom: height - PAD,
    };

    if kind == ItemKind::Emoji {
        let color_ok = renderer::draw_color_emoji_scaled(
            hdc as *mut std::ffi::c_void,
            width,
            height,
            &content,
            inner.left,
            inner.top,
            inner.right,
            inner.bottom,
            font_px as f32,
        );
        if !color_ok {
            draw_centered(hdc, kind, font_px, &content, inner, palette.text);
        }
    } else if kind == ItemKind::Symbol {
        draw_centered(hdc, kind, font_px, &content, inner, palette.text);
    } else if kind == ItemKind::Snippet {
        draw_wrapped(hdc, kind, font_px, &content, inner, palette.text);
    } else {
        draw_exact_lines(hdc, kind, font_px, &content, width, height, palette.text);
    }

    if buffered {
        BitBlt(screen, 0, 0, width, height, mem, 0, 0, SRCCOPY);
        SelectObject(mem, old_bitmap);
        DeleteObject(bitmap as _);
        DeleteDC(mem);
    } else if !mem.is_null() {
        DeleteDC(mem);
    }

    EndPaint(hwnd, &ps);
}

unsafe fn draw_centered(
    hdc: HDC,
    kind: ItemKind,
    font_px: i32,
    content: &str,
    mut rect: RECT,
    color: COLORREF,
) {
    let font = create_preview_font(kind, font_px);
    if font.is_null() {
        return;
    }
    let old = SelectObject(hdc, font as _);
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, color);
    let mut text: Vec<u16> = content.encode_utf16().collect();
    DrawTextW(
        hdc,
        text.as_mut_ptr(),
        text.len() as i32,
        &mut rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
    SelectObject(hdc, old);
    DeleteObject(font as _);
}

unsafe fn draw_wrapped(
    hdc: HDC,
    kind: ItemKind,
    font_px: i32,
    content: &str,
    bounds: RECT,
    color: COLORREF,
) {
    let font = create_preview_font(kind, font_px);
    if font.is_null() {
        return;
    }
    let old = SelectObject(hdc, font as _);
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, color);

    let normalized = normalize_content(content);
    let mut measured_text: Vec<u16> = normalized.encode_utf16().collect();
    let mut measured = RECT { left: 0, top: 0, right: (bounds.right - bounds.left).max(1), bottom: 0 };
    DrawTextW(
        hdc,
        measured_text.as_mut_ptr(),
        measured_text.len() as i32,
        &mut measured,
        DT_LEFT | DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX | DT_CALCRECT,
    );
    let block_h = (measured.bottom - measured.top).max(1);
    let available_h = (bounds.bottom - bounds.top).max(1);
    let mut target = bounds;
    if block_h < available_h {
        target.top += (available_h - block_h) / 2;
        target.bottom = target.top + block_h;
    }

    let mut text: Vec<u16> = normalized.encode_utf16().collect();
    DrawTextW(
        hdc,
        text.as_mut_ptr(),
        text.len() as i32,
        &mut target,
        DT_LEFT | DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX,
    );

    SelectObject(hdc, old);
    DeleteObject(font as _);
}

unsafe fn draw_exact_lines(
    hdc: HDC,
    kind: ItemKind,
    font_px: i32,
    content: &str,
    width: i32,
    height: i32,
    color: COLORREF,
) {
    let font = create_preview_font(kind, font_px);
    if font.is_null() {
        return;
    }
    let old = SelectObject(hdc, font as _);
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, color);

    let normalized = normalize_content(content);
    let lines: Vec<&str> = normalized.split('\n').collect();
    let (block_w, block_h) = measure_exact(kind, &normalized, font_px);
    let line_height = (block_h / lines.len().max(1) as i32).max(1);
    let x = ((width - block_w) / 2).max(PAD);
    let y = ((height - block_h) / 2).max(PAD);

    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let text: Vec<u16> = line.encode_utf16().collect();
        TextOutW(
            hdc,
            x,
            y + index as i32 * line_height,
            text.as_ptr(),
            text.len() as i32,
        );
    }

    SelectObject(hdc, old);
    DeleteObject(font as _);
}

unsafe fn create_preview_font(kind: ItemKind, font_px: i32) -> HFONT {
    let family = match kind {
        ItemKind::Emoji => "Segoe UI Emoji",
        ItemKind::Symbol | ItemKind::Kaomoji => "Segoe UI Symbol",
        ItemKind::Ascii => "Consolas",
        ItemKind::Snippet => "Segoe UI",
    };
    let family = wide(family);
    CreateFontW(
        -font_px.max(1),
        0,
        0,
        0,
        FW_NORMAL as i32,
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

fn normalize_content(content: &str) -> String {
    content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ")
}

unsafe fn fill(hdc: HDC, rect: RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    if brush.is_null() {
        return;
    }
    FillRect(hdc, &rect, brush);
    DeleteObject(brush as _);
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
