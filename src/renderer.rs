use std::{cell::RefCell, ffi::c_void};

use windows::{
    core::{w, PCWSTR, Result as WinResult},
    Win32::{
        Foundation::{FALSE, RECT},
        Graphics::{
            Direct2D::{
                Common::{D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D_RECT_F},
                D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_RENDER_TARGET_PROPERTIES,
            },
            DirectWrite::{
                DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
                DWRITE_WORD_WRAPPING_NO_WRAP,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::HDC,
        },
    },
};

pub struct EmojiDraw<'a> {
    pub text: &'a str,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

thread_local! {
    static COLOR_RENDERER: RefCell<Option<ColorEmojiRenderer>> = const { RefCell::new(None) };
}

/// Draws emoji on top of the GDI-painted popup using Direct2D + DirectWrite.
/// Returns false when color rendering is unavailable so the caller can fall
/// back to monochrome GDI rather than leaving a blank cell.
pub unsafe fn draw_color_emojis(
    raw_hdc: *mut c_void,
    width: i32,
    height: i32,
    draws: &[EmojiDraw<'_>],
) -> bool {
    if draws.is_empty() {
        return true;
    }

    COLOR_RENDERER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = ColorEmojiRenderer::new().ok();
        }

        let Some(renderer) = slot.as_ref() else {
            return false;
        };

        renderer.draw(raw_hdc, width, height, draws).is_ok()
    })
}

/// Same color-font path as the picker grid, but with a caller-selected font
/// size. The preview window uses this to make a single emoji fill its content
/// without switching back to monochrome GDI.
pub unsafe fn draw_color_emoji_scaled(
    raw_hdc: *mut c_void,
    width: i32,
    height: i32,
    text: &str,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    font_size: f32,
) -> bool {
    COLOR_RENDERER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = ColorEmojiRenderer::new().ok();
        }

        let Some(renderer) = slot.as_ref() else {
            return false;
        };

        renderer
            .draw_scaled(
                raw_hdc,
                width,
                height,
                text,
                left,
                top,
                right,
                bottom,
                font_size,
            )
            .is_ok()
    })
}

struct ColorEmojiRenderer {
    target: ID2D1DCRenderTarget,
    format: IDWriteTextFormat,
    brush: windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
}

impl ColorEmojiRenderer {
    unsafe fn new() -> WinResult<Self> {
        let d2d: ID2D1Factory =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;

        let mut props = D2D1_RENDER_TARGET_PROPERTIES::default();
        props.pixelFormat.format = DXGI_FORMAT_B8G8R8A8_UNORM;
        props.pixelFormat.alphaMode = D2D1_ALPHA_MODE_IGNORE;
        let target = d2d.CreateDCRenderTarget(&props)?;

        let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        let family = preferred_emoji_font(&dwrite);
        let format = create_emoji_format(&dwrite, &family, 27.0)?;

        let white = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let brush = target.CreateSolidColorBrush(&white, None)?;

        Ok(Self {
            target,
            format,
            brush,
        })
    }

    unsafe fn draw(
        &self,
        raw_hdc: *mut c_void,
        width: i32,
        height: i32,
        draws: &[EmojiDraw<'_>],
    ) -> WinResult<()> {
        self.draw_with_format(raw_hdc, width, height, draws, &self.format)
    }

    unsafe fn draw_scaled(
        &self,
        raw_hdc: *mut c_void,
        width: i32,
        height: i32,
        text: &str,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        font_size: f32,
    ) -> WinResult<()> {
        let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        let family = preferred_emoji_font(&dwrite);
        let format = create_emoji_format(&dwrite, &family, font_size.max(8.0))?;
        let draw = EmojiDraw {
            text,
            left,
            top,
            right,
            bottom,
        };
        self.draw_with_format(raw_hdc, width, height, std::slice::from_ref(&draw), &format)
    }

    unsafe fn draw_with_format(
        &self,
        raw_hdc: *mut c_void,
        width: i32,
        height: i32,
        draws: &[EmojiDraw<'_>],
        format: &IDWriteTextFormat,
    ) -> WinResult<()> {
        let bounds = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        self.target.BindDC(HDC(raw_hdc), &bounds)?;
        self.target.BeginDraw();

        for draw in draws {
            let utf16: Vec<u16> = draw.text.encode_utf16().collect();
            let rect = D2D_RECT_F {
                left: draw.left as f32,
                top: draw.top as f32,
                right: draw.right as f32,
                bottom: draw.bottom as f32,
            };
            self.target.DrawText(
                &utf16,
                format,
                &rect,
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }

        self.target.EndDraw(None, None)
    }
}

unsafe fn create_emoji_format(
    factory: &IDWriteFactory,
    family: &[u16],
    font_size: f32,
) -> WinResult<IDWriteTextFormat> {
    let format = factory.CreateTextFormat(
        PCWSTR(family.as_ptr()),
        None::<&IDWriteFontCollection>,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_STRETCH_NORMAL,
        font_size,
        w!("en-us"),
    )?;
    format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
    format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
    format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
    Ok(format)
}

unsafe fn preferred_emoji_font(factory: &IDWriteFactory) -> Vec<u16> {
    let noto = wide("Noto Color Emoji");
    let mut collection = None;
    if factory
        .GetSystemFontCollection(&mut collection, false)
        .is_ok()
    {
        if let Some(collection) = collection {
            let mut index = 0u32;
            let mut exists = FALSE;
            if collection
                .FindFamilyName(PCWSTR(noto.as_ptr()), &mut index, &mut exists)
                .is_ok()
                && exists.as_bool()
            {
                return noto;
            }
        }
    }

    // Segoe UI Emoji is shipped with Windows 10/11 and is the safest native
    // fallback. We intentionally do not redistribute Apple Color Emoji.
    wide("Segoe UI Emoji")
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
