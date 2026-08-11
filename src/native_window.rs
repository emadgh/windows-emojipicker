use std::mem::{size_of, zeroed};

use windows_sys::Win32::{
    Foundation::{HMONITOR, HWND, LPARAM, LRESULT, POINT, RECT},
    Graphics::Gdi::CreateRoundRectRgn,
    UI::{
        Shell::{Shell_NotifyIconGetRect, NOTIFYICONIDENTIFIER},
        WindowsAndMessaging::*,
    },
};

/// Reusable base behavior for small frameless native Win32 utility windows.
///
/// This type deliberately contains no application-specific state. It can be
/// copied into other Rust/Win32 tools to provide consistent rounded-window
/// geometry, monitor-safe placement, and background dragging.
#[derive(Clone, Copy, Debug)]
pub struct NativeWindowBase {
    pub width: i32,
    pub height: i32,
    pub radius: i32,
}

impl NativeWindowBase {
    pub const fn new(width: i32, height: i32, radius: i32) -> Self {
        Self { width, height, radius }
    }

    /// Applies the shared rounded frameless shape.
    pub unsafe fn apply_rounding(self, hwnd: HWND) {
        let region = CreateRoundRectRgn(
            0,
            0,
            self.width.saturating_add(1),
            self.height.saturating_add(1),
            self.radius,
            self.radius,
        );
        if !region.is_null() {
            SetWindowRgn(hwnd, region, 1);
        }
    }

    /// Native drag behavior for frameless windows. Application code only
    /// declares which client-space points are interactive; every other point
    /// becomes HTCAPTION and is draggable using the standard Windows mover.
    pub unsafe fn drag_hit_test(
        self,
        hwnd: HWND,
        lparam: LPARAM,
        is_interactive: impl FnOnce(i32, i32) -> bool,
    ) -> Option<LRESULT> {
        let (screen_x, screen_y) = point_from_lparam(lparam);
        let mut point = POINT { x: screen_x, y: screen_y };
        if ScreenToClient(hwnd, &mut point) == 0 {
            return None;
        }
        if !(0..self.width).contains(&point.x) || !(0..self.height).contains(&point.y) {
            return None;
        }

        Some(if is_interactive(point.x, point.y) {
            HTCLIENT as LRESULT
        } else {
            HTCAPTION as LRESULT
        })
    }

    /// Centers a child utility window over its owner and clamps it to the
    /// owner's monitor work area.
    pub unsafe fn centered_on_owner(self, owner: HWND, gap: i32) -> (i32, i32) {
        let mut owner_rect: RECT = zeroed();
        let owner_valid = !owner.is_null()
            && GetWindowRect(owner, &mut owner_rect) != 0
            && rect_is_usable(owner_rect);

        let monitor = if owner_valid {
            MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST)
        } else {
            MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY)
        };
        let work = monitor_work_area(monitor).unwrap_or(RECT {
            left: 0,
            top: 0,
            right: GetSystemMetrics(SM_CXSCREEN),
            bottom: GetSystemMetrics(SM_CYSCREEN),
        });

        let (mut x, mut y) = if owner_valid {
            (
                owner_rect.left + ((owner_rect.right - owner_rect.left) - self.width) / 2,
                owner_rect.top + ((owner_rect.bottom - owner_rect.top) - self.height) / 2,
            )
        } else {
            (
                work.left + ((work.right - work.left) - self.width) / 2,
                work.top + ((work.bottom - work.top) - self.height) / 2,
            )
        };
        clamp_to_work_area(&mut x, &mut y, self.width, self.height, work, gap);
        (x, y)
    }

    /// GahYar-compatible tray-click placement: use the mouse position at the
    /// exact moment the tray icon is clicked, center horizontally on it, and
    /// place the popup immediately above the bottom edge of the monitor work
    /// area. This intentionally mirrors GahYar instead of guessing a corner.
    pub unsafe fn above_taskbar_at_cursor(self, gap: i32) -> (i32, i32) {
        let mut cursor: POINT = zeroed();
        if GetCursorPos(&mut cursor) == 0 {
            cursor = POINT {
                x: GetSystemMetrics(SM_CXSCREEN) - gap,
                y: GetSystemMetrics(SM_CYSCREEN) - gap,
            };
        }
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let work = monitor_work_area(monitor).unwrap_or(RECT {
            left: 0,
            top: 0,
            right: GetSystemMetrics(SM_CXSCREEN),
            bottom: GetSystemMetrics(SM_CYSCREEN),
        });

        let mut x = cursor.x - self.width / 2;
        let mut y = work.bottom - self.height - gap;
        clamp_to_work_area(&mut x, &mut y, self.width, self.height, work, gap);
        (x, y)
    }

    /// Hotkey fallback when no text caret can be resolved. Unlike the tray
    /// click path this does not depend on where the mouse currently is: it asks
    /// Explorer for the actual notification-icon rectangle.
    pub unsafe fn above_tray_icon(self, hwnd: HWND, icon_id: u32, gap: i32) -> Option<(i32, i32)> {
        let mut identifier: NOTIFYICONIDENTIFIER = zeroed();
        identifier.cbSize = size_of::<NOTIFYICONIDENTIFIER>() as u32;
        identifier.hWnd = hwnd;
        identifier.uID = icon_id;

        let mut icon: RECT = zeroed();
        if Shell_NotifyIconGetRect(&identifier, &mut icon) < 0 || !rect_is_usable(icon) {
            return None;
        }

        let center = POINT {
            x: icon.left + (icon.right - icon.left) / 2,
            y: icon.top + (icon.bottom - icon.top) / 2,
        };
        let monitor = MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST);
        let work = monitor_work_area(monitor)?;
        let mut x = center.x - self.width / 2;
        let mut y = work.bottom - self.height - gap;
        clamp_to_work_area(&mut x, &mut y, self.width, self.height, work, gap);
        Some((x, y))
    }

    /// Places a popup near an anchor such as a text caret and keeps it inside
    /// the monitor work area.
    pub unsafe fn near_anchor(self, anchor: POINT, gap: i32) -> (i32, i32) {
        let monitor = MonitorFromPoint(anchor, MONITOR_DEFAULTTONEAREST);
        let work = monitor_work_area(monitor).unwrap_or(RECT {
            left: 0,
            top: 0,
            right: anchor.x + self.width,
            bottom: anchor.y + self.height,
        });

        let mut x = anchor.x;
        let mut y = anchor.y + gap;
        if y + self.height > work.bottom {
            y = anchor.y - self.height - gap;
        }
        clamp_to_work_area(&mut x, &mut y, self.width, self.height, work, 0);
        (x, y)
    }
}

unsafe fn monitor_work_area(monitor: HMONITOR) -> Option<RECT> {
    if monitor.is_null() {
        return None;
    }
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        None
    } else {
        Some(info.rcWork)
    }
}

fn clamp_to_work_area(
    x: &mut i32,
    y: &mut i32,
    width: i32,
    height: i32,
    work: RECT,
    gap: i32,
) {
    let min_x = work.left.saturating_add(gap);
    let min_y = work.top.saturating_add(gap);
    let max_x = work.right.saturating_sub(width).saturating_sub(gap).max(min_x);
    let max_y = work.bottom.saturating_sub(height).saturating_sub(gap).max(min_y);
    *x = (*x).clamp(min_x, max_x);
    *y = (*y).clamp(min_y, max_y);
}

fn rect_is_usable(rect: RECT) -> bool {
    rect.right > rect.left && rect.bottom > rect.top
}

pub fn point_from_lparam(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam as u32 & 0xffff) as u16 as i16 as i32;
    let y = ((lparam as u32 >> 16) & 0xffff) as u16 as i16 as i32;
    (x, y)
}
