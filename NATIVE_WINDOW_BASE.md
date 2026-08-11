# NativeWindowBase

`src/native_window.rs` is the reusable frameless-window base used by Windows Emoji Picker.

It intentionally has no dependency on picker state, themes, emoji data, custom items, or update logic, so it can be copied into future native Rust/Win32 utilities.

## Responsibilities

- Rounded frameless window region.
- Native background dragging through `WM_NCHITTEST` / `HTCAPTION`.
- Owner-centered dialog placement with monitor work-area clamping.
- GahYar-compatible system-tray click placement using the cursor position at the exact tray click.
- Notification-icon based placement for non-mouse fallbacks such as a global hotkey.
- Caret/anchor popup placement with work-area clamping.

## Usage pattern

```rust
const BASE: NativeWindowBase = NativeWindowBase::new(WIDTH, HEIGHT, 18);

// After CreateWindowExW:
BASE.apply_rounding(hwnd);

// Frameless dragging:
WM_NCHITTEST => {
    if let Some(hit) = BASE.drag_hit_test(hwnd, lparam, |x, y| {
        point_is_interactive(x, y)
    }) {
        return hit;
    }
}
```

`point_is_interactive` should return `true` for buttons, cards, inputs, links, tabs, or any other region that must receive normal mouse input. Every other part of the client background becomes draggable.

## Tray placement

For a real tray click, call:

```rust
let (x, y) = BASE.above_taskbar_at_cursor(8);
```

This intentionally mirrors GahYar: the popup is horizontally centered on the mouse while the mouse is over the tray icon, and vertically aligned to the bottom of the monitor work area.

For a hotkey fallback where the mouse may be elsewhere, use:

```rust
let position = BASE.above_tray_icon(hwnd, TRAY_ICON_ID, 8);
```

## Re-entrancy rule for Win32 UI state

Do not hold a `RefCell::borrow_mut()` across synchronous Win32 calls such as `SetWindowTextW`, `SetFocus`, `SendMessageW`, or dialog creation. Those functions may synchronously re-enter a window procedure. Copy HWND/state values first, release the borrow, then call Win32. The custom-item manager follows this rule to avoid runtime borrow panics.
