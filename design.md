# GahYar Native Rust Design System

این سند تم پایه برای برنامه‌های Native ویندوزی نوشته‌شده با Rust است. منبع بصری آن پروژه GahYar است و هدف این است که ابزارهای مستقل Win32 ظاهر یکپارچه داشته باشند، بدون WebView، Electron یا runtime مرورگر.

## اصول اصلی

- UI کاملاً Native با Win32، GDI/Direct2D و DirectWrite.
- ظاهر آرام، متراکم و کاربردی؛ بدون گرادیان، شیشه‌ای‌سازی یا animation غیرضروری.
- پنجره‌های popup و dialog با گوشه‌های گرد `18px`.
- کنترل‌های کوچک با radius حدود `9–12px`.
- فضای اصلی برنامه از `background` و کارت‌ها/کنترل‌ها از `surface` استفاده کنند.
- رنگ طلایی Accent فقط برای انتخاب، CTA، نسخه، نام سازنده و نقاط مهم استفاده شود.
- رنگ آبی `event` برای لینک و وضعیت اطلاعاتی.
- رنگ قرمز `danger` فقط برای خطا، حذف یا هشدار.
- هر frame در memory DC رندر شود و در انتها یک‌باره با `BitBlt` نمایش داده شود تا flicker ایجاد نشود.
- `WM_ERASEBKGND` برای پنجره‌های custom-painted مقدار handled برگرداند.
- از polling و repaint مداوم خودداری شود؛ فقط روی state change پنجره invalidate شود.

## پالت رنگ

### Dark

| Token | RGB | کاربرد |
|---|---:|---|
| `background` | `30, 31, 33` | پس‌زمینه اصلی |
| `surface` | `43, 45, 48` | کارت و کنترل |
| `surface_alt` | `53, 56, 60` | hover / input / secondary button |
| `text` | `240, 241, 243` | متن اصلی |
| `muted` | `176, 183, 192` | متن ثانویه |
| `faint` | `91, 97, 104` | متن بسیار کم‌اهمیت |
| `accent` | `248, 211, 88` | انتخاب، CTA، برند |
| `accent_text` | `24, 24, 24` | متن روی Accent |
| `border` | `70, 70, 70` | مرزها |
| `event` | `126, 180, 255` | لینک و اطلاعات |
| `selected` | `64, 58, 38` | سطح انتخاب‌شده |
| `danger` | `255, 104, 104` | خطا و حذف |

### Light

| Token | RGB | کاربرد |
|---|---:|---|
| `background` | `231, 233, 236` | پس‌زمینه اصلی |
| `surface` | `250, 250, 250` | کارت و کنترل |
| `surface_alt` | `241, 243, 246` | hover / input / secondary button |
| `text` | `27, 31, 36` | متن اصلی |
| `muted` | `91, 98, 107` | متن ثانویه |
| `faint` | `176, 181, 188` | متن بسیار کم‌اهمیت |
| `accent` | `230, 181, 43` | انتخاب، CTA، برند |
| `accent_text` | `29, 25, 12` | متن روی Accent |
| `border` | `211, 214, 219` | مرزها |
| `event` | `32, 100, 191` | لینک و اطلاعات |
| `selected` | `255, 246, 205` | سطح انتخاب‌شده |
| `danger` | `206, 49, 49` | خطا و حذف |

## Typography

ترتیب پیشنهادی فونت:

1. `Vazirmatn` برای رابط فارسی، اگر برنامه آن را به‌صورت مجاز embed کرده باشد.
2. `Segoe UI` به‌عنوان fallback عمومی ویندوز.
3. `Segoe UI Symbol` برای symbol/kaomoji.
4. `Consolas` برای ASCII art و متن monospace.
5. `Noto Color Emoji` در صورت نصب بودن، سپس `Segoe UI Emoji` برای Emoji رنگی.

اندازه‌های پایه:

- Tiny/footer: `11–12px`
- Small/control: `13px`
- Body: `15–16px`
- Heading: `22–23px`, bold
- Emoji grid: حدود `27px`

در UI فارسی از `DT_RTLREADING` استفاده شود. نام‌ها و metadata جستجو لازم نیست در کارت‌ها نمایش داده شوند مگر واقعاً به تصمیم کاربر کمک کنند.

## Geometry

- عرض ابزارهای popup کوچک: حدود `400–440px`.
- Outer padding: `12–18px`.
- فاصله کنترل‌ها: `6–10px`.
- Radius پنجره: `18px`.
- Radius کارت/دکمه: `9–12px`.
- نوار footer: حدود `26–30px`.
- popup باید به work area مانیتور clamp شود و از caret یا cursor خارج نشود.

## Tabs

- tab فعال: `accent` + `accent_text`.
- tab غیرفعال: `surface` + `muted`.
- tabها گوشه گرد داشته باشند.
- tabهای غیرکاربردی مانند `All` فقط زمانی اضافه شوند که واقعاً workflow را بهتر کنند.
- اگر آیتم‌ها self-explanatory هستند، عنوان آیتم زیر preview نمایش داده نشود؛ عنوان/keyword برای Search باقی بماند.

## Cards and Lists

حالت پایه:

- normal: `surface`
- hover: `surface_alt`
- selected: `selected`
- radius: `9–11px`

Emoji فقط glyph را نشان دهد. Kaomoji و Symbol centered باشند. Text می‌تواند چندخطی باشد. ASCII باید با فونت monospace و حفظ line break نمایش داده شود.

## Inputs

- background: `surface_alt`
- text: `text`
- placeholder: `muted`
- ارتفاع معمول: `30–44px`
- border سنگین استفاده نشود؛ contrast سطح کافی است.

## Footer / Branding

الگوی GahYar:

- سمت راست: `عماد قاسمی - emadghasemi.ir`
- سمت چپ: `App Name vX.Y.Z`
- هر دو با رنگ `accent` و فونت tiny.
- footer باید کم‌ارتفاع باشد و از محتوای اصلی توجه نگیرد.

## About Window

ابعاد پایه نزدیک `380 × 300`، radius `18px`.

ترتیب محتوا:

1. نام برنامه با Accent و فونت title.
2. `عماد قاسمی - emadghasemi.ir` با Accent.
3. آدرس GitHub با `event`.
4. نسخه برنامه با `muted`.
5. دکمه وضعیت بروزرسانی با `surface_alt` و متن Accent.
6. دکمه بستن.

وب‌سایت و GitHub clickable باشند.

## Update UX

الگوی رفتاری GahYar:

- در startup یک بار latest GitHub Release بررسی شود.
- هر ۶ ساعت دوباره بررسی شود.
- اگر `auto_update` فعال است، نسخه جدید خودکار دانلود شود.
- EXE جدید در temp ذخیره و اعتبار اولیه `MZ` بررسی شود.
- updater مخفی PowerShell بعد از بسته‌شدن process فایل فعلی را جایگزین کند و برنامه را دوباره اجرا کند.
- حالت‌ها: `Idle`, `Checking`, `UpToDate`, `Available`, `Downloading`, `Failed`.
- About و footer باید status را بدون blocking UI نمایش دهند.

## Theme Switching

- Theme باید persistent باشد.
- تغییر تم بلافاصله تمام پنجره‌های باز (main, manager, about) را invalidate کند.
- کنترل‌های custom-painted و edit brushها باید با palette جدید بازسازی شوند.
- Dark تم پیش‌فرض است.

## Rendering / Flicker Rules

برای پنجره‌های custom-painted:

1. `BeginPaint` روی screen DC.
2. `CreateCompatibleDC` و `CreateCompatibleBitmap`.
3. کل frame روی memory DC رسم شود.
4. DirectWrite/Direct2D overlay نیز روی همان memory DC انجام شود.
5. یک `BitBlt` نهایی به screen DC.
6. bitmap/DC آزاد شوند.
7. `WM_ERASEBKGND` handled باشد.
8. hover فقط وقتی item تغییر کرده invalidate کند.
9. scroll فقط وقتی offset تغییر کرده invalidate کند.

## Native Architecture Recommendation

```text
UI / Win32 Windows
        ↓
Application State / Services
        ↓
Domain Models
        ↓
Persistence + Windows Adapters
```

Theme، Update، Storage و Custom Item Manager ماژول مستقل باشند و UI اصلی مستقیماً منطق network/storage را پیاده نکند.

## Release Rule

هر تغییر user-facing باید version را در `Cargo.toml` افزایش دهد. Release build باید `windows-emojipicker.exe` را به‌عنوان GitHub Release asset با همین نام منتشر کند تا updater بتواند آن را پیدا کند.
