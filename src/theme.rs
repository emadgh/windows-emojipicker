use windows_sys::Win32::Foundation::COLORREF;

use crate::settings::Theme;

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: COLORREF,
    pub surface: COLORREF,
    pub surface_alt: COLORREF,
    pub text: COLORREF,
    pub muted: COLORREF,
    pub faint: COLORREF,
    pub accent: COLORREF,
    pub accent_text: COLORREF,
    pub border: COLORREF,
    pub event: COLORREF,
    pub selected: COLORREF,
    pub danger: COLORREF,
}

impl Palette {
    pub fn current() -> Self {
        Self::from_theme(crate::settings::get().theme)
    }

    pub fn from_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self {
                background: rgb(30, 31, 33),
                surface: rgb(43, 45, 48),
                surface_alt: rgb(53, 56, 60),
                text: rgb(240, 241, 243),
                muted: rgb(176, 183, 192),
                faint: rgb(91, 97, 104),
                accent: rgb(248, 211, 88),
                accent_text: rgb(24, 24, 24),
                border: rgb(70, 70, 70),
                event: rgb(126, 180, 255),
                selected: rgb(64, 58, 38),
                danger: rgb(255, 104, 104),
            },
            Theme::Light => Self {
                background: rgb(231, 233, 236),
                surface: rgb(250, 250, 250),
                surface_alt: rgb(241, 243, 246),
                text: rgb(27, 31, 36),
                muted: rgb(91, 98, 107),
                faint: rgb(176, 181, 188),
                accent: rgb(230, 181, 43),
                accent_text: rgb(29, 25, 12),
                border: rgb(211, 214, 219),
                event: rgb(32, 100, 191),
                selected: rgb(255, 246, 205),
                danger: rgb(206, 49, 49),
            },
        }
    }
}

pub const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    red as u32 | (green as u32) << 8 | (blue as u32) << 16
}
