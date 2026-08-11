fn preview_text(item: PickerItem) -> String {
    if matches!(item.kind, ItemKind::Ascii) {
        item.content
            .lines()
            .next()
            .unwrap_or(item.content)
            .to_string()
    } else {
        item.content.replace('\r', "").replace('\n', " ↵ ")
    }
}

unsafe fn fill(hdc: HDC, rect: &RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    if !brush.is_null() {
        FillRect(hdc, rect, brush);
        DeleteObject(brush as _);
    }
}

unsafe fn draw_text(
    hdc: HDC,
    font: HFONT,
    text: &str,
    rect: &mut RECT,
    format: u32,
    color: COLORREF,
) {
    if font.is_null() {
        return;
    }
    let old = SelectObject(hdc, font as _);
    SetTextColor(hdc, color);
    let mut buffer: Vec<u16> = text.encode_utf16().collect();
    DrawTextW(
        hdc,
        buffer.as_mut_ptr(),
        buffer.len() as i32,
        rect,
        format,
    );
    if !old.is_null() {
        SelectObject(hdc, old);
    }
}

unsafe fn add_tray_icon(hwnd: HWND) -> bool {
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ICON_ID;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = LoadIconW(null_mut(), IDI_APPLICATION);
    copy_wide_fixed("Windows Emoji Picker", &mut data.szTip);

    Shell_NotifyIconW(NIM_ADD, &data) != 0
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ICON_ID;
    Shell_NotifyIconW(NIM_DELETE, &data);
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }

    let open = wide("Open picker\tWin+Shift+.");
    let exit = wide("Exit");
    AppendMenuW(menu, MF_STRING, CMD_OPEN, open.as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(menu, MF_STRING, CMD_EXIT, exit.as_ptr());

    let mut point: POINT = zeroed();
    GetCursorPos(&mut point);
    SetForegroundWindow(hwnd);
    TrackPopupMenu(
        menu,
        TPM_RIGHTBUTTON,
        point.x,
        point.y,
        0,
        hwnd,
        null(),
    );
    DestroyMenu(menu);
}

fn copy_wide_fixed<const N: usize>(text: &str, output: &mut [u16; N]) {
    output.fill(0);
    for (dst, src) in output
        .iter_mut()
        .take(N.saturating_sub(1))
        .zip(text.encode_utf16())
    {
        *dst = src;
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn point_from_lparam(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam as u32 & 0xffff) as u16 as i16 as i32;
    let y = ((lparam as u32 >> 16) & 0xffff) as u16 as i16 as i32;
    (x, y)
}

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
}
