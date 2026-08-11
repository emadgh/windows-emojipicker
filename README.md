# windows-emojipicker

A lightweight, **fully native Windows** emoji / kaomoji / ASCII art / ready-text picker written in Rust.

No WebView. No Electron. No Tauri. No HTML/CSS/JavaScript runtime.

The MVP uses raw Win32 APIs for the window, system tray, global hotkey, caret lookup, input injection and GDI rendering. The only Rust dependency is Microsoft's low-level `windows-sys` bindings.

## MVP features

- Runs quietly in the Windows system tray.
- Global hotkey: **Win + Shift + .**
- Remembers the foreground target window before opening.
- Attempts to place the popup beside the active text caret using `GetGUIThreadInfo`.
- Falls back to the mouse position when a caret is not exposed.
- Clamps the popup to the current monitor work area.
- Native dark popup (`WS_POPUP`, tool-window, top-most while visible).
- Categories:
  - Emoji
  - Kaomoji
  - ASCII art
  - Ready text
  - Symbols
- Persian + English keyword search.
- Keyboard navigation:
  - Type to search
  - Arrow keys to move
  - `Enter` to insert
  - `Esc` to close
  - `Tab` to switch category
- Mouse selection and wheel scrolling.
- Unicode insertion using Win32 `SendInput` + `KEYEVENTF_UNICODE`.
- Refuses to inject if the original target window could not be restored, preventing text from being sent to an unexpected app.

## Native stack

```text
Rust
  |
  +-- Win32 window/message loop
  +-- RegisterHotKey
  +-- Shell_NotifyIconW (system tray)
  +-- GetGUIThreadInfo / ClientToScreen (caret position)
  +-- GDI (MVP renderer)
  +-- SendInput (Unicode insertion)
  +-- windows-sys (bindings only)
```

`windows-sys` adds declarations and bindings; it does not embed a UI/runtime engine into the executable.

## Build

Requirements on Windows:

- Rust stable (MSVC toolchain recommended)
- Visual Studio Build Tools / MSVC + Windows SDK

```powershell
cargo build --release
```

The executable will be created at:

```text
target\release\windows-emojipicker.exe
```

Run it once. It will remain in the notification area. Press `Win + Shift + .` while editing text in another application.

## Current project layout

```text
src/
├── main.rs     Windows-only entry point
├── app.rs      Win32 message loop, popup UI, tray, hotkey, caret and insertion
├── model.rs    picker domain types
└── data.rs     built-in MVP emoji/kaomoji/ASCII/snippet/symbol catalog
```

## MVP limitations / next work

This first version deliberately keeps the surface area small. The following are planned rather than hidden behind extra frameworks:

1. UI Automation (`IUIAutomationTextPattern2::GetCaretRange`) as the primary caret provider, with the current `GetGUIThreadInfo` path kept as fallback.
2. Direct2D + DirectWrite renderer for better color emoji, typography, animation and DPI scaling while remaining fully native.
3. Favorites and recents.
4. User-defined snippets and categories persisted under `%LOCALAPPDATA%`.
5. Configurable global hotkey.
6. Optional “Start with Windows”.
7. Better IME/search editing and RTL presentation.
8. Clipboard-paste strategy for applications that do not accept `KEYEVENTF_UNICODE` well.
9. App icon, installer and signed releases.

### Elevated applications

Windows UIPI can prevent a normal-integrity process from injecting input into an application running as Administrator. The picker should normally run non-elevated; insertion into an elevated target may therefore fail by design.

### Hotkey conflicts

`RegisterHotKey` fails if another program or Windows owns the requested shortcut. The MVP reports an error at launch in that case. Configurable hotkeys are planned.

## Development principles

- Windows-native only.
- Zero WebView / browser engine.
- Idle path should do no continuous rendering.
- Avoid polling when an event/message can be used.
- Keep platform code isolated from picker data/domain logic.
- Never inject text into a foreground window different from the captured target.

## License

MIT
