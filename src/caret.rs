use std::mem::size_of;

use windows::Win32::{
    System::{
        Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED},
        Ole::SafeArrayDestroy,
    },
    UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern2, UIA_TextPattern2Id,
    },
};

/// Returns the screen-space caret anchor for the currently focused text control.
/// UI Automation TextPattern2 is used first because Chromium/Electron/UWP and
/// many modern editors do not expose a useful Win32 system caret.
pub fn focused_caret_point() -> Option<(i32, i32)> {
    unsafe {
        // Ignore the HRESULT: if COM is already initialized in another model,
        // COM calls on the thread are still valid in that existing apartment.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let focused = automation.GetFocusedElement().ok()?;
        let pattern: IUIAutomationTextPattern2 =
            focused.GetCurrentPatternAs(UIA_TextPattern2Id).ok()?;

        let mut active = Default::default();
        let range = pattern.GetCaretRange(&mut active).ok()?;
        if !active.as_bool() {
            return None;
        }

        let rectangles = range.GetBoundingRectangles().ok()?;
        if rectangles.is_null() {
            return None;
        }

        let result = {
            let array = &*rectangles;
            let count = if array.cDims == 1 {
                array.rgsabound[0].cElements as usize
            } else {
                0
            };

            if count >= 4
                && array.cbElements as usize == size_of::<f64>()
                && !array.pvData.is_null()
            {
                let values = std::slice::from_raw_parts(array.pvData as *const f64, count);
                let left = values[0];
                let top = values[1];
                let width = values[2];
                let height = values[3];
                if left.is_finite() && top.is_finite() && width.is_finite() && height.is_finite() {
                    Some((left.round() as i32, (top + height.max(1.0)).round() as i32))
                } else {
                    None
                }
            } else {
                None
            }
        };

        let _ = SafeArrayDestroy(rectangles);
        result
    }
}
