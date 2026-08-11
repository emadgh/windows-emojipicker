use std::{cell::RefCell, mem::zeroed, ptr::{null, null_mut}};

use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::VK_ESCAPE,
        Shell::ShellExecuteW,
        WindowsAndMessaging::*,
    },
};

use crate::{theme::Palette, update};

const CLASS_NAME: &str = "WindowsEmojiPicker.About";
const W: i32 = 380;
const H: i32 = 300;
const WEBSITE_URL: &str = "https://emadghasemi.ir";
const GITHUB_URL: &str = "https://github.com/emadgh/windows-emojipicker";

thread_local! {
    static STATE: RefCell<AboutState> = RefCell::new(AboutState::default());
}

struct AboutState {
    hwnd: HWND,
    owner: HWND,
    request_update_message: u32,
    title_font: HFONT,
    font: HFONT,
    small_font: HFONT,
}

impl Default for AboutState {
    fn default() -> Self {
        Self { hwnd: null_mut(), owner: null_mut(), request_update_message: 0, title_font: null_mut(), font: null_mut(), small_font: null_mut() }
    }
}

pub unsafe fn show(owner: HWND, request_update_message: u32) {
    let existing = STATE.with(|cell| cell.borrow().hwnd);
    if !existing.is_null() && IsWindow(existing) != 0 {
        ShowWindow(existing, SW_SHOW);
        SetForegroundWindow(existing);
        return;
    }

    let instance = GetModuleHandleW(null());
    let class_name = wide(CLASS_NAME);
    let mut wc: WNDCLASSW = zeroed();
    wc.lpfnWndProc = Some(wnd_proc);
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    wc.hbrBackground = null_mut();
    wc.lpszClassName = class_name.as_ptr();
    RegisterClassW(&wc);

    let mut owner_rect: RECT = zeroed();
    GetWindowRect(owner, &mut owner_rect);
    let x = owner_rect.left + ((owner_rect.right - owner_rect.left) - W) / 2;
    let y = owner_rect.top + ((owner_rect.bottom - owner_rect.top) - H) / 2;
    let title = wide("درباره Emoji Picker");
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class_name.as_ptr(), title.as_ptr(),
        WS_POPUP,
        x.max(0), y.max(0), W, H,
        owner, null_mut(), instance, null(),
    );
    if hwnd.is_null() { return; }

    STATE.with(|cell| {
        *cell.borrow_mut() = AboutState {
            hwnd,
            owner,
            request_update_message,
            title_font: create_font(-23, FW_BOLD as i32, "Segoe UI"),
            font: create_font(-16, FW_NORMAL as i32, "Segoe UI"),
            small_font: create_font(-13, FW_NORMAL as i32, "Segoe UI"),
        };
    });

    let region = CreateRoundRectRgn(0, 0, W + 1, H + 1, 18, 18);
    SetWindowRgn(hwnd, region, 1);
    ShowWindow(hwnd, SW_SHOW);
    SetForegroundWindow(hwnd);
}

pub unsafe fn invalidate() {
    let hwnd = STATE.with(|cell| cell.borrow().hwnd);
    if !hwnd.is_null() && IsWindow(hwnd) != 0 { InvalidateRect(hwnd, null(), 0); }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => { paint(hwnd); return 0; }
        WM_LBUTTONUP => {
            let (x, y) = point_from_lparam(lparam);
            if (34..346).contains(&x) && (58..94).contains(&y) {
                open_url(hwnd, WEBSITE_URL);
                return 0;
            }
            if (34..346).contains(&x) && (96..132).contains(&y) {
                open_url(hwnd, GITHUB_URL);
                return 0;
            }
            if (70..310).contains(&x) && (184..224).contains(&y) {
                let (owner, message) = STATE.with(|cell| {
                    let state = cell.borrow();
                    (state.owner, state.request_update_message)
                });
                PostMessageW(owner, message, 0, 0);
                return 0;
            }
            if (140..240).contains(&x) && (244..278).contains(&y) {
                DestroyWindow(hwnd);
                return 0;
            }
        }
        WM_KEYDOWN if wparam as u16 == VK_ESCAPE => { DestroyWindow(hwnd); return 0; }
        WM_CLOSE => { DestroyWindow(hwnd); return 0; }
        WM_DESTROY => {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                if !state.title_font.is_null() { DeleteObject(state.title_font as _); }
                if !state.font.is_null() { DeleteObject(state.font as _); }
                if !state.small_font.is_null() { DeleteObject(state.small_font as _); }
                *state = AboutState::default();
            });
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    if screen.is_null() { return; }
    let mem = CreateCompatibleDC(screen);
    let bitmap = CreateCompatibleBitmap(screen, W, H);
    let old_bitmap = SelectObject(mem, bitmap as _);
    let palette = Palette::current();
    fill(mem, &RECT { left: 0, top: 0, right: W, bottom: H }, palette.surface);

    let (title_font, font, small_font) = STATE.with(|cell| {
        let state = cell.borrow();
        (state.title_font, state.font, state.small_font)
    });
    draw_text(mem, title_font, "Emoji Picker،", RECT { left: 50, top: 14, right: 330, bottom: 52 }, palette.accent, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_RTLREADING);
    draw_text(mem, font, "عماد قاسمی - emadghasemi.ir", RECT { left: 34, top: 58, right: 346, bottom: 94 }, palette.accent, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_RTLREADING);
    draw_text(mem, small_font, "github.com/emadgh/windows-emojipicker", RECT { left: 34, top: 96, right: 346, bottom: 132 }, palette.event, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    draw_text(mem, small_font, &format!("نسخه {}", env!("CARGO_PKG_VERSION")), RECT { left: 34, top: 136, right: 346, bottom: 172 }, palette.muted, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_RTLREADING);

    let update_text = match update::status() {
        update::UpdateStatus::Checking => "در حال بررسی…",
        update::UpdateStatus::Downloading => "در حال بروزرسانی…",
        update::UpdateStatus::UpToDate => "برنامه بروز است",
        update::UpdateStatus::Available(_) => "دریافت نسخه جدید",
        update::UpdateStatus::Failed(_) => "تلاش دوباره برای بروزرسانی",
        _ => "بررسی بروزرسانی",
    };
    round_fill(mem, RECT { left: 70, top: 184, right: 310, bottom: 224 }, palette.surface_alt, 12);
    draw_text(mem, small_font, update_text, RECT { left: 70, top: 184, right: 310, bottom: 224 }, palette.accent, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_RTLREADING);
    round_fill(mem, RECT { left: 140, top: 244, right: 240, bottom: 278 }, palette.surface_alt, 10);
    draw_text(mem, small_font, "بستن", RECT { left: 140, top: 244, right: 240, bottom: 278 }, palette.text, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_RTLREADING);

    BitBlt(screen, 0, 0, W, H, mem, 0, 0, SRCCOPY);
    SelectObject(mem, old_bitmap);
    DeleteObject(bitmap as _);
    DeleteDC(mem);
    EndPaint(hwnd, &ps);
}

unsafe fn open_url(hwnd: HWND, url: &str) {
    let operation = wide("open");
    let url = wide(url);
    ShellExecuteW(hwnd, operation.as_ptr(), url.as_ptr(), null(), null(), SW_SHOWNORMAL);
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

fn point_from_lparam(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam as u32 & 0xffff) as u16 as i16 as i32;
    let y = ((lparam as u32 >> 16) & 0xffff) as u16 as i16 as i32;
    (x, y)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
