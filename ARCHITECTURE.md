# Architecture notes

## Goal

A resident Windows utility that opens a small picker beside the current text caret, lets the user find an emoji/kaomoji/ASCII/snippet/symbol, inserts it into the previously active application, then disappears.

## Constraints

- Rust implementation.
- Windows only.
- Fully native UI and OS integration.
- No WebView, browser runtime, HTML, JS, Electron or Tauri.
- Very low idle CPU/RAM overhead.

## Current MVP boundaries

### `model.rs`
Owns stable domain concepts (`ItemKind`, `PickerItem`). It has no Win32 dependency.

### `data.rs`
Static built-in catalog. Keeping the initial catalog compiled into the binary avoids database startup cost and filesystem failure modes.

### `app.rs`
The Windows adapter plus current MVP presentation layer. It owns:

- window class / HWND lifecycle;
- message loop;
- tray icon and tray menu;
- global hotkey;
- caret/mouse anchor resolution;
- monitor-aware popup placement;
- GDI rendering and hit-testing;
- search/category/selection state;
- foreground-target restoration;
- Unicode `SendInput` insertion.

The next refactor should split `app.rs` into `platform/windows`, `ui`, and `services` only after behavior is verified on real Windows applications. Avoid premature abstraction around Win32 calls whose behavior is still being tested.

## Event flow

```text
Win+Shift+.
    |
    v
WM_HOTKEY
    |
    +--> capture foreground HWND
    +--> query caret rectangle / cursor fallback
    +--> clamp popup to monitor work area
    +--> show + focus picker

keyboard / mouse
    |
    +--> update query/category/selection
    +--> InvalidateRect (event-driven repaint)

Enter / click item
    |
    +--> capture selected static content
    +--> hide picker
    +--> restore captured foreground HWND
    +--> verify it is foreground
    +--> SendInput UTF-16 units
```

## Rendering direction

The MVP intentionally uses GDI because it is built into Windows and lets the interaction model be validated with minimal code/dependencies. Once behavior is stable, replace only the renderer with Direct2D/DirectWrite. Hotkey, tray, focus, caret and insertion logic should remain unchanged.

## Safety invariant

The application must never type into an unexpected window. Before insertion, the saved target HWND is validated, restored to foreground, and checked again. If restoration fails, insertion is aborted.
