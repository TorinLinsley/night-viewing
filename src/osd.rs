// 快捷键提示与托盘小面板：置顶无边框浮层
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateFontW, CreatePen, CreateRoundRectRgn, CreateSolidBrush,
    DEFAULT_CHARSET, DEFAULT_PITCH, DeleteDC, DeleteObject, DrawTextW, Ellipse, EndPaint, FillRect,
    GetStockObject, HDC, HFONT, InvalidateRect, LineTo, MoveToEx, NULL_BRUSH, NULL_PEN,
    OUT_TT_PRECIS, PAINTSTRUCT, Polygon, PS_SOLID, SRCCOPY, SelectObject, SetBkMode, SetTextColor,
    TRANSPARENT, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect,
    GetCursorPos, GetMessageW, GetWindowLongPtrW, GetWindowRect, IsWindowVisible, LoadCursorW,
    PostMessageW, PostQuitMessage,
    RegisterClassW, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, TranslateMessage, GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW, LWA_ALPHA,
    MSG, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WM_DESTROY,
    WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_TIMER,
};

use crate::config::{MAX_LEVEL, OsdMode};
use crate::core::Core;
use crate::i18n::{self, Lang};
use crate::winapi_util;

const OSD_CLASS: &str = "NightViewing::Hint";
const WM_HINT: u32 = 0x8001;
const WM_PANEL: u32 = 0x8002;
const WM_PANEL_HIDE: u32 = 0x8003;

const LM_W: f32 = 348.0;
const LM_PAD: f32 = 14.0;
const LM_ROW: f32 = 46.0;

const WM_CAPTURECHANGED: u32 = 0x0215;
const WM_MOUSEACTIVATE: u32 = 0x0021;
const MA_NOACTIVATE: isize = 3;

static HINT_WND: AtomicIsize = AtomicIsize::new(0);
static PANEL_WND: AtomicIsize = AtomicIsize::new(0);
static CORE: OnceLock<Arc<Core>> = OnceLock::new();
static DARK: AtomicBool = AtomicBool::new(true);

pub fn init(core: &Arc<Core>, dark: bool) {
    let _ = CORE.set(core.clone());
    DARK.store(dark, Ordering::Relaxed);
    std::thread::Builder::new()
        .name("night-viewing-hint".into())
        .spawn(run)
        .ok();
}

pub fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
    for slot in [&HINT_WND, &PANEL_WND] {
        let h = slot.load(Ordering::SeqCst);
        if h != 0 {
            unsafe {
                let _ = InvalidateRect(Some(HWND(h as *mut _)), None, false);
            }
        }
    }
}

fn post(slot: &AtomicIsize, msg: u32) {
    let h = slot.load(Ordering::SeqCst);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), msg, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn notify_hint() {
    if let Some(core) = CORE.get() {
        if core.snapshot().0.osd == OsdMode::Off {
            return;
        }
    }
    post(&HINT_WND, WM_HINT);
}

pub fn toggle_panel() {
    post(&PANEL_WND, WM_PANEL);
}

pub fn hide_panel() {
    post(&PANEL_WND, WM_PANEL_HIDE);
}

pub fn raise() {
    for slot in [&HINT_WND, &PANEL_WND] {
        let h = slot.load(Ordering::Relaxed);
        if h == 0 {
            continue;
        }
        let hwnd = HWND(h as *mut _);
        unsafe {
            if IsWindowVisible(hwnd).as_bool() {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn cref(c: u32) -> COLORREF {
    COLORREF(((c & 0x0000FF) << 16) | (c & 0x00FF00) | ((c & 0xFF0000) >> 16))
}

fn cur_scale(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        winapi_util::system_scale()
    } else {
        dpi as f32 / 96.0
    }
}

struct Style {
    card: u32,
    border: u32,
    text: u32,
    dim: u32,
    accent: u32,
    track: u32,
    knob: u32,
}

fn style(dark: bool) -> Style {
    if dark {
        Style {
            card: 0x1B1B21,
            border: 0x33333C,
            text: 0xECECF1,
            dim: 0x9A9AA6,
            accent: 0x5B7CFA,
            track: 0x393942,
            knob: 0xFFFFFF,
        }
    } else {
        Style {
            card: 0xFFFFFF,
            border: 0xD7D7DE,
            text: 0x14141A,
            dim: 0x5F5F6A,
            accent: 0x3F63F0,
            track: 0xDCDCE3,
            knob: 0xFFFFFF,
        }
    }
}

struct Paint {
    d: HDC,
    s: f32,
    fonts: Vec<(i32, isize)>,
}

impl Paint {
    fn new(d: HDC, s: f32) -> Self {
        Self { d, s, fonts: Vec::new() }
    }

    fn px(&self, v: f32) -> i32 {
        (v * self.s).round() as i32
    }

    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> RECT {
        RECT {
            left: self.px(x),
            top: self.px(y),
            right: self.px(x + w),
            bottom: self.px(y + h),
        }
    }

    fn font(&mut self, size: f32, bold: bool) -> HFONT {
        let h = self.px(size).max(9);
        let key = h + if bold { 1000 } else { 0 };
        if let Some(v) = self.fonts.iter().find(|(k, _)| *k == key) {
            return HFONT(v.1 as *mut core::ffi::c_void);
        }
        let f = crate::winapi_util::cached_font(h, bold);
        self.fonts.push((key, f.0 as isize));
        f
    }

    fn fill(&self, r: RECT, color: u32) {
        unsafe {
            let b = CreateSolidBrush(cref(color));
            let _ = FillRect(self.d, &r, b);
            let _ = DeleteObject(b.into());
        }
    }

    fn round(&self, r: RECT, rad: f32, color: u32) {
        let pts = round_points(r, self.px(rad));
        unsafe {
            let b = CreateSolidBrush(cref(color));
            let old_b = SelectObject(self.d, b.into());
            let old_p = SelectObject(self.d, GetStockObject(NULL_PEN));
            let _ = Polygon(self.d, &pts);
            SelectObject(self.d, old_p);
            SelectObject(self.d, old_b);
            let _ = DeleteObject(b.into());
        }
    }

    fn stroke(&self, r: RECT, rad: f32, color: u32, width: f32) {
        let pts = round_points(r, self.px(rad));
        let w = self.px(width).max(1);
        unsafe {
            let p = CreatePen(PS_SOLID, w, cref(color));
            let old_p = SelectObject(self.d, p.into());
            let old_b = SelectObject(self.d, GetStockObject(NULL_BRUSH));
            let _ = Polygon(self.d, &pts);
            SelectObject(self.d, old_b);
            SelectObject(self.d, old_p);
            let _ = DeleteObject(p.into());
        }
    }

    fn circle(&self, cx: f32, cy: f32, rad: f32, color: u32) {
        let (x, y, rr) = (self.px(cx), self.px(cy), self.px(rad));
        unsafe {
            let b = CreateSolidBrush(cref(color));
            let old_b = SelectObject(self.d, b.into());
            let old_p = SelectObject(self.d, GetStockObject(NULL_PEN));
            let _ = Ellipse(self.d, x - rr, y - rr, x + rr, y + rr);
            SelectObject(self.d, old_p);
            SelectObject(self.d, old_b);
            let _ = DeleteObject(b.into());
        }
    }

    fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: u32) {
        let w = self.px(width).max(1);
        unsafe {
            let p = CreatePen(PS_SOLID, w, cref(color));
            let old_p = SelectObject(self.d, p.into());
            let _ = MoveToEx(self.d, self.px(x1), self.px(y1), None);
            let _ = LineTo(self.d, self.px(x2), self.px(y2));
            SelectObject(self.d, old_p);
            let _ = DeleteObject(p.into());
        }
    }

    fn text(&mut self, r: RECT, s: &str, size: f32, color: u32, fmt: u32, bold: bool) {
        if s.is_empty() {
            return;
        }
        let f = self.font(size, bold);
        let mut buf: Vec<u16> = s.encode_utf16().collect();
        let mut rc = r;
        unsafe {
            let old_f = SelectObject(self.d, f.into());
            SetBkMode(self.d, TRANSPARENT);
            SetTextColor(self.d, cref(color));
            DrawTextW(
                self.d,
                &mut buf,
                &mut rc,
                windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT(fmt),
            );
            SelectObject(self.d, old_f);
        }
    }
}

const FMT_LEFT: u32 = 0x0000 | 0x0004 | 0x0020 | 0x0800;
const FMT_RIGHT: u32 = 0x0002 | 0x0004 | 0x0020 | 0x0800;

fn round_points(r: RECT, rad: i32) -> Vec<POINT> {
    let max_r = ((r.right - r.left) / 2).min((r.bottom - r.top) / 2);
    let rad = rad.clamp(0, max_r.max(0));
    if rad <= 0 {
        return vec![
            POINT { x: r.left, y: r.top },
            POINT { x: r.right, y: r.top },
            POINT { x: r.right, y: r.bottom },
            POINT { x: r.left, y: r.bottom },
        ];
    }
    let seg = 8;
    let half = std::f32::consts::FRAC_PI_2;
    let corners = [
        (r.right - rad, r.top + rad, -half),
        (r.right - rad, r.bottom - rad, 0.0),
        (r.left + rad, r.bottom - rad, half),
        (r.left + rad, r.top + rad, half * 2.0),
    ];
    let mut pts = Vec::with_capacity(48);
    for (cx, cy, start) in corners {
        for i in 0..=seg {
            let a = start + half * (i as f32 / seg as f32);
            pts.push(POINT {
                x: cx + (a.cos() * rad as f32) as i32,
                y: cy + (a.sin() * rad as f32) as i32,
            });
        }
    }
    pts
}

struct Row {
    hit: RECT,
    track: RECT,
    device: Option<String>,
}

struct State {
    core: Arc<Core>,
    lang: Lang,
    panel: bool,
    drag: Option<usize>,
    hide_at: Instant,
    rev: u64,
    rows: Vec<Row>,
    monitors: Vec<winapi_util::Monitor>,
    dark: bool,
    click_prev: bool,
    rgn: (i32, i32),
}

unsafe fn monitor_icon(p: &mut Paint, x: f32, y: f32, size: f32, color: u32) {
    let screen = p.r(x, y, size, size * 0.66);
    p.stroke(screen, 2.0, color, 1.3);
    let cx = x + size / 2.0;
    p.line(cx, y + size * 0.66, cx, y + size * 0.9, 1.3, color);
    p.line(cx - size * 0.26, y + size * 0.9, cx + size * 0.26, y + size * 0.9, 1.3, color);
}

unsafe fn paint(hwnd: HWND, st: &mut State) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let (w, h) = (client.right, client.bottom);
    if w <= 0 || h <= 0 {
        let _ = EndPaint(hwnd, &ps);
        return;
    }
    let s = cur_scale(hwnd);
    let mem = CreateCompatibleDC(Some(hdc));
    let bmp = CreateCompatibleBitmap(hdc, w, h);
    let old_bmp = SelectObject(mem, bmp.into());
    let mut p = Paint::new(mem, s);
    let sty = style(st.dark);
    let lw = w as f32 / s;
    p.fill(
        RECT { left: 0, top: 0, right: w, bottom: h },
        sty.card,
    );
    let border = p.r(0.5, 0.5, lw - 1.0, h as f32 / s - 1.0);
    p.stroke(border, 12.0, sty.border, 1.0);

    let (cfg, rev) = st.core.snapshot();
    st.lang = cfg.lang;
    st.rev = rev;
    let row_h = LM_ROW;
    let mut rows = Vec::with_capacity(1 + st.monitors.len());

    for i in 0..=(st.monitors.len()) {
        let top = LM_PAD + i as f32 * row_h;
        let cy = top + row_h / 2.0;
        let (label, device, value, enabled) = if i == 0 {
            (i18n::ui(cfg.lang, "全部显示器"), None, cfg.opacity, true)
        } else {
            let m = &st.monitors[i - 1];
            (
                i18n::tr(cfg.lang, &format!("显示器 {i}")),
                Some(m.device.clone()),
                cfg.level_for(&m.device),
                cfg.display_on(&m.device),
            )
        };

        monitor_icon(&mut p, LM_PAD + 2.0, cy - 7.5, 15.0, if enabled { sty.dim } else { sty.track });
        let lab = p.r(LM_PAD + 26.0, top, 88.0, row_h);
        p.text(
            lab,
            &label,
            11.5,
            if enabled { sty.text } else { sty.dim },
            FMT_LEFT,
            i == 0,
        );

        let track_x = LM_PAD + 118.0;
        let track_w = lw - track_x - LM_PAD - 48.0;
        let track = p.r(track_x, cy - 2.0, track_w, 4.0);
        p.round(track, 2.0, sty.track);
        let frac = (value / MAX_LEVEL).clamp(0.0, 1.0);
        if frac > 0.0 {
            let filled = p.r(track_x, cy - 2.0, track_w * frac, 4.0);
            p.round(filled, 2.0, if enabled { sty.accent } else { sty.track });
        }
        let knob_x = track_x + track_w * frac;
        p.circle(knob_x, cy, 7.0, if enabled { sty.knob } else { sty.track });
        p.circle(knob_x, cy, 5.0, if enabled { sty.accent } else { sty.track });

        let val = p.r(lw - LM_PAD - 44.0, top, 40.0, row_h);
        p.text(
            val,
            &format!("{}", value.round() as i32),
            12.0,
            if enabled { sty.text } else { sty.dim },
            FMT_RIGHT,
            true,
        );

        rows.push(Row {
            hit: RECT {
                left: p.px(LM_PAD * 0.4),
                top: p.px(top),
                right: w - p.px(LM_PAD * 0.4),
                bottom: p.px(top + row_h),
            },
            track,
            device,
        });
    }
    st.rows = rows;

    let _ = BitBlt(hdc, 0, 0, w, h, Some(mem), 0, 0, SRCCOPY);
    SelectObject(mem, old_bmp);
    let _ = DeleteObject(bmp.into());
    let _ = DeleteDC(mem);
    let _ = EndPaint(hwnd, &ps);
}

unsafe fn layout(hwnd: HWND, st: &mut State) {
    let s = cur_scale(hwnd);
    let count = st.monitors.len().max(1);
    let rows = 1 + count;
    let w = (LM_W * s).round() as i32;
    let h = ((LM_PAD * 2.0 + LM_ROW * rows as f32) * s).round() as i32;
    let work = winapi_util::work_area();
    let gap = (16.0 * s).round() as i32;
    let x = (work.right - w - gap).max(work.left);
    let y = (work.bottom - h - gap).max(work.top);
    let rad = (12.0 * s).round() as i32;
    unsafe {
        if st.rgn != (w, h) {
            let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, rad, rad);
            let _ = windows::Win32::Graphics::Gdi::SetWindowRgn(hwnd, Some(rgn), true);
            st.rgn = (w, h);
        }
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE,
        );
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 250, LWA_ALPHA);
    }
}

unsafe fn show(hwnd: HWND, st: &mut State, millis: u64) {
    st.monitors = winapi_util::monitors();
    let (cfg, _) = st.core.snapshot();
    st.lang = cfg.lang;
    st.dark = DARK.load(Ordering::Relaxed);
    unsafe {
        layout(hwnd, st);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = InvalidateRect(Some(hwnd), None, false);
        let _ = UpdateWindow(hwnd);
    }
    st.hide_at = Instant::now() + Duration::from_millis(millis);
}

unsafe fn set_from_x(hwnd: HWND, st: &mut State, idx: usize, x: i32) {
    let Some(row) = st.rows.get(idx) else {
        return;
    };
    let s = cur_scale(hwnd);
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let lw = client.right as f32 / s;
    let track_x = LM_PAD + 118.0;
    let track_w = (lw - track_x - LM_PAD - 48.0).max(24.0);
    let (l, r) = (track_x * s, (track_x + track_w) * s);
    let frac = (((x as f32) - l) / (r - l).max(1.0)).clamp(0.0, 1.0);
    let value = (frac * MAX_LEVEL * 10.0).round() / 10.0;
    let device = row.device.clone();
    st.core.set_level(device.as_deref(), value);
    st.hide_at = Instant::now() + Duration::from_millis(3000);
}

fn hit(rows: &[Row], x: i32, y: i32) -> Option<usize> {
    rows.iter().position(|r| {
        x >= r.hit.left && x <= r.hit.right && y >= r.hit.top && y <= r.hit.bottom
    })
}

fn point(lparam: LPARAM) -> (i32, i32) {
    let raw = lparam.0 as i32;
    let x = (raw & 0xFFFF) as i16 as i32;
    let y = ((raw >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

unsafe extern "system" fn osd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let st = &mut *ptr;
        match msg {
            WM_HINT => {
                if let Some(core) = CORE.get() {
                    if core.snapshot().0.osd == OsdMode::Off {
                        return LRESULT(0);
                    }
                }
                show(hwnd, st, 2800);
                LRESULT(0)
            }
            WM_PANEL => {
                if IsWindowVisible(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                } else {
                    show(hwnd, st, 7000);
                }
                LRESULT(0)
            }
            WM_PANEL_HIDE => {
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }
            WM_ERASEBKGND => {
                let hdc = HDC(wparam.0 as *mut _);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let brush = CreateSolidBrush(cref(style(st.dark).card));
                FillRect(hdc, &rc, brush);
                let _ = DeleteObject(brush.into());
                LRESULT(1)
            }
            WM_PAINT => {
                paint(hwnd, st);
                LRESULT(0)
            }
            WM_TIMER => {
                if IsWindowVisible(hwnd).as_bool() {
                    let down = (GetAsyncKeyState(0x01) as u16 & 0x8000) != 0
                        || (GetAsyncKeyState(0x02) as u16 & 0x8000) != 0;
                    if down && !st.click_prev && st.drag.is_none() {
                        let mut pt = POINT::default();
                        let _ = GetCursorPos(&mut pt);
                        let mut rc = RECT::default();
                        let _ = GetWindowRect(hwnd, &mut rc);
                        let inside = pt.x >= rc.left
                            && pt.x < rc.right
                            && pt.y >= rc.top
                            && pt.y < rc.bottom;
                        if !inside {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                            st.hide_at = Instant::now();
                        }
                    }
                    st.click_prev = down;
                }
                let (cfg, rev) = st.core.snapshot();
                if rev != st.rev {
                    st.rev = rev;
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    let _ = UpdateWindow(hwnd);
                }
                let visible = IsWindowVisible(hwnd).as_bool();
                if visible {
                    if cfg.osd == OsdMode::Force || cfg.always_on_top {
                        let _ = SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    }
                    if st.drag.is_none() && Instant::now() >= st.hide_at {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let (x, y) = point(lparam);
                if let Some(idx) = st.drag {
                    set_from_x(hwnd, st, idx, x);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    let _ = UpdateWindow(hwnd);
                } else if hit(&st.rows, x, y).is_some() {
                    st.hide_at = Instant::now() + Duration::from_millis(2000);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let (x, y) = point(lparam);
                if let Some(idx) = hit(&st.rows, x, y) {
                    st.drag = Some(idx);
                    let _ = SetCapture(hwnd);
                    set_from_x(hwnd, st, idx, x);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    let _ = UpdateWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP | WM_CAPTURECHANGED => {
                if st.drag.is_some() {
                    let _ = ReleaseCapture();
                    st.drag = None;
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    let _ = UpdateWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE),
            WM_DESTROY => {
                drop(Box::from_raw(ptr));
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn run() {
    unsafe {
        let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let Some(core) = CORE.get() else {
            return;
        };
        let hinstance = HINSTANCE(GetModuleHandleW(None).map(|m| m.0).unwrap_or_default());
        let class = wide(OSD_CLASS);
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(osd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: Default::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: CreateSolidBrush(COLORREF(0x00221B1E)),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(class.as_ptr()),
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }
        let ex = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE;
        let title = wide("Night Viewing Hint");
        let (cfg, _) = core.snapshot();
        for (panel, slot) in [(false, &HINT_WND), (true, &PANEL_WND)] {
            let Ok(hwnd) = CreateWindowExW(
                ex,
                PCWSTR(class.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP,
                0,
                0,
                10,
                10,
                None,
                None,
                Some(hinstance),
                None,
            ) else {
                return;
            };
            let state = Box::new(State {
                core: core.clone(),
                lang: cfg.lang,
                panel,
                drag: None,
                hide_at: Instant::now(),
                rev: 0,
                rows: Vec::new(),
                monitors: winapi_util::monitors(),
                dark: DARK.load(Ordering::Relaxed),
                click_prev: false,
                rgn: (0, 0),
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 250, LWA_ALPHA);
            SetTimer(Some(hwnd), 1, 40, None);
            slot.store(hwnd.0 as isize, Ordering::SeqCst);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
