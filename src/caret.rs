use std::{cell::Cell, mem::size_of};

use windows::Win32::{
    System::{
        Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED},
        Ole::SafeArrayDestroy,
    },
    UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern2, IUIAutomationTextRange,
        TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start, TextUnit_Character,
        UIA_TextPattern2Id,
    },
};

thread_local! {
    static COM_INITIALIZED: Cell<bool> = const { Cell::new(false) };
}

#[derive(Clone, Copy, Debug)]
struct TextRect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl TextRect {
    fn right(self) -> f64 {
        self.left + self.width
    }

    fn bottom(self) -> f64 {
        self.top + self.height
    }

    fn center_y(self) -> f64 {
        self.top + self.height / 2.0
    }

    fn is_plausible(self) -> bool {
        self.left.is_finite()
            && self.top.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= 0.0
            && self.height > 0.5
            && self.width < 20_000.0
            && self.height < 20_000.0
            && self.left.abs() < 1_000_000.0
            && self.top.abs() < 1_000_000.0
    }
}

/// Returns the screen-space caret anchor for the currently focused text control.
///
/// The direct TextPattern2 caret range is zero-length. Some UI Automation
/// providers return a useful caret rectangle for it, while others correctly
/// return an empty rectangle collection. For the latter case we probe one
/// character forward/backward and infer the insertion boundary from the two
/// adjacent character rectangles. This works for modern Chromium/WebView,
/// WinUI/UWP and editor controls that do not expose a legacy Win32 caret.
pub fn focused_caret_point() -> Option<(i32, i32)> {
    unsafe {
        ensure_com_initialized();

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

        // A few providers expose a real non-empty caret rectangle directly.
        if let Some(rect) = first_valid_rect(&range) {
            // Reject the common invalid sentinel rectangle at the screen origin.
            if !(rect.left.abs() < 0.5
                && rect.top.abs() < 0.5
                && rect.width.abs() < 0.5
                && rect.height <= 1.0)
            {
                return Some((rect.left.round() as i32, rect.bottom().round() as i32));
            }
        }

        let forward = probe_forward(&range);
        let backward = probe_backward(&range);
        infer_caret_from_neighbors(backward, forward)
    }
}

unsafe fn ensure_com_initialized() {
    COM_INITIALIZED.with(|initialized| {
        if !initialized.get() {
            // RPC_E_CHANGED_MODE is harmless here: it means this thread already
            // has a COM apartment and UI Automation can use that apartment.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            initialized.set(true);
        }
    });
}

unsafe fn probe_forward(range: &IUIAutomationTextRange) -> Option<TextRect> {
    let probe = range.Clone().ok()?;
    let moved = probe
        .MoveEndpointByUnit(TextPatternRangeEndpoint_End, TextUnit_Character, 1)
        .ok()?;
    if moved == 0 {
        return None;
    }
    first_valid_rect(&probe)
}

unsafe fn probe_backward(range: &IUIAutomationTextRange) -> Option<TextRect> {
    let probe = range.Clone().ok()?;
    let moved = probe
        .MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -1)
        .ok()?;
    if moved == 0 {
        return None;
    }
    first_valid_rect(&probe)
}

fn infer_caret_from_neighbors(
    backward: Option<TextRect>,
    forward: Option<TextRect>,
) -> Option<(i32, i32)> {
    match (backward, forward) {
        (Some(previous), Some(next)) => {
            let same_line = (previous.center_y() - next.center_y()).abs()
                <= previous.height.max(next.height) * 0.75;

            if same_line {
                // This is direction-agnostic. In LTR text previous.right and
                // next.left meet at the caret; in RTL the opposite pair meets.
                let previous_edges = [previous.left, previous.right()];
                let next_edges = [next.left, next.right()];
                let mut best = (f64::INFINITY, previous.right(), next.left);
                for a in previous_edges {
                    for b in next_edges {
                        let distance = (a - b).abs();
                        if distance < best.0 {
                            best = (distance, a, b);
                        }
                    }
                }
                let x = ((best.1 + best.2) / 2.0).round() as i32;
                let y = previous.bottom().max(next.bottom()).round() as i32;
                return Some((x, y));
            }

            // A line wrap/newline can put the adjacent character rectangles on
            // different lines. The forward character is the best anchor for
            // the new visual line in that case.
            Some((next.left.round() as i32, next.bottom().round() as i32))
        }
        (None, Some(next)) => Some((next.left.round() as i32, next.bottom().round() as i32)),
        (Some(previous), None) => {
            Some((previous.right().round() as i32, previous.bottom().round() as i32))
        }
        (None, None) => None,
    }
}

unsafe fn first_valid_rect(range: &IUIAutomationTextRange) -> Option<TextRect> {
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
            values
                .chunks_exact(4)
                .map(|value| TextRect {
                    left: value[0],
                    top: value[1],
                    width: value[2],
                    height: value[3],
                })
                .find(|rect| rect.is_plausible())
        } else {
            None
        }
    };

    let _ = SafeArrayDestroy(rectangles);
    result
}
