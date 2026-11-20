// 原生 GDI 界面：窗口、绘制、鼠标与键盘交互
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Arc;
use std::time::Instant;

use tray_icon::TrayIcon;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush, LineTo, MoveToEx, DEFAULT_CHARSET, DEFAULT_PITCH,
    DeleteObject, DrawTextW, Ellipse, EndPaint, GetMonitorInfoW, GetStockObject, GetTextExtentPoint32W,
    HDC, HFONT, HGDIOBJ, HMONITOR, InvalidateRect, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
    MONITOR_DEFAULTTONEAREST,
    NULL_BRUSH, NULL_PEN, OUT_TT_PRECIS, PAINTSTRUCT, PS_SOLID, Polygon,
    SRCCOPY, SelectObject, SetBkMode, SetTextColor, SetViewportOrgEx, TRANSPARENT, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_BACK, VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_RIGHT, VK_TAB, VK_UP, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRect, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    GWLP_USERDATA, SetWindowTextW, GetClientRect, GetMessageW, GetWindowLongPtrW, IDC_ARROW, LoadCursorW, MINMAXINFO, MSG,
    GetCaretBlinkTime, PostQuitMessage, RegisterClassW, SW_SHOW, SWP_NOMOVE, SWP_NOZORDER, SetTimer, SetWindowLongPtrW,
    GetCursorPos, SWP_NOSIZE, SetWindowPos, ShowWindow, SystemParametersInfoW, TranslateMessage,
    SPI_GETWORKAREA,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WM_CHAR, WM_CLOSE, WM_CREATE, WM_DESTROY,
    WM_ENTERSIZEMOVE, WM_ERASEBKGND, WM_EXITSIZEMOVE, WM_GETMINMAXINFO, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_SETICON, WM_SIZE, WM_SYSKEYDOWN,
    GCLP_HICON, GCLP_HICONSM, ICON_BIG, ICON_SMALL, ICON_SMALL2, GetSystemMetrics,
    SM_CXICON, SM_CXSMICON, SetClassLongPtrW,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_SETTINGCHANGE, WM_TIMER, WNDCLASSW, WS_EX_APPWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPEDWINDOW,
    WS_SYSMENU,
};

use crate::config::{Config, MonKey, OsdMode, ThemeMode, MAX_LEVEL};
use crate::core::Core;
use crate::i18n::{self, Lang, LANGS};
use crate::icon;
use crate::winapi_util;

const CLASS: &str = "NightViewing::Main";
const W: f32 = 508.0;
const DEFAULT_HEIGHT: f32 = 959.0;
const PAD: f32 = 24.0;

const CHIPS: [(&str, f32); 4] = [("柔光", 15.0), ("适中", 30.0), ("深暗", 45.0), ("极暗", 60.0)];
const T_TRAY: u8 = 0;
const T_AUTO: u8 = 1;
const T_RESTORE: u8 = 2;
const T_HOTKEY: u8 = 3;
const T_PASS: u8 = 4;
const T_TOP: u8 = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Id {
    Slider,
    Input,
    Chip(usize),
    Seg(usize),
    Toggle(u8),
    HkChip(u8),
    MonHead(usize),
    MonKey(usize, usize),
    MonAct(usize, usize),
    MonActItem(usize, usize, u8),
    MonDel(usize, usize),
    MonAdd(usize),
    MonStep(usize, usize, usize),
    MonStepIn(usize, usize),
    StepUp(usize),
    StepDown(usize),
    StepIn(u8),
    Osd(usize),
    Btn(u8),
    Display(usize),
    Lang,
    LangItem(usize),
}

#[derive(Clone, Copy)]
struct Pal {
    bg: u32,
    card: u32,
    stroke: u32,
    text: u32,
    dim: u32,
    accent: u32,
    track: u32,
    knob: u32,
}

fn pal(dark: bool) -> Pal {
    if dark {
        Pal {
            bg: 0x111114,
            card: 0x1A1A1E,
            stroke: 0x2E2E36,
            text: 0xEEEEF2,
            dim: 0x96969E,
            accent: 0x6E96FF,
            track: 0x3A3A42,
            knob: 0xFFFFFF,
        }
    } else {
        Pal {
            bg: 0xF3F3F6,
            card: 0xFFFFFF,
            stroke: 0xE4E4EA,
            text: 0x16161A,
            dim: 0x74747E,
            accent: 0x2E68E8,
            track: 0xD6D6DC,
            knob: 0xFFFFFF,
        }
    }
}

fn cref(v: u32) -> COLORREF {
    COLORREF(((v & 0xFF) << 16) | (v & 0xFF00) | ((v >> 16) & 0xFF))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

const FMT_LEFT: u32 = 0x0000 | 0x0004 | 0x0020 | 0x0800;
const FMT_RIGHT: u32 = 0x0002 | 0x0004 | 0x0020 | 0x0800;
const FMT_CENTER: u32 = 0x0001 | 0x0004 | 0x0020 | 0x0800;

struct Draw {
    d: HDC,
    s: f32,
    w: f32,
    lang: Lang,
    fonts: HashMap<i32, isize>,
}

impl Draw {
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
        if let Some(v) = self.fonts.get(&key) {
            return HFONT(*v as *mut c_void);
        }
        let face = wide("Microsoft YaHei UI");
        let f = unsafe {
            CreateFontW(
                -h,
                0,
                0,
                0,
                if bold { 700 } else { 400 },
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_TT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                DEFAULT_PITCH.0 as u32,
                PCWSTR(face.as_ptr()),
            )
        };
        self.fonts.insert(key, f.0 as isize);
        f
    }

    fn fill(&self, r: RECT, color: u32) {
        unsafe {
            let b = CreateSolidBrush(cref(color));
            let _ = windows::Win32::Graphics::Gdi::FillRect(self.d, &r, b);
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

    fn ring(&self, cx: f32, cy: f32, rad: f32, width: f32, color: u32) {
        let (x, y, rr) = (self.px(cx), self.px(cy), self.px(rad));
        let w = self.px(width).max(1);
        unsafe {
            let p = CreatePen(PS_SOLID, w, cref(color));
            let old_p = SelectObject(self.d, p.into());
            let old_b = SelectObject(self.d, GetStockObject(NULL_BRUSH));
            let _ = Ellipse(self.d, x - rr, y - rr, x + rr, y + rr);
            SelectObject(self.d, old_b);
            SelectObject(self.d, old_p);
            let _ = DeleteObject(p.into());
        }
    }

    fn chevron(&self, cx: f32, cy: f32, half: f32, down: bool, color: u32) {
        let (x, y, hh) = (self.px(cx), self.px(cy), self.px(half));
        let w = self.px(1.7).max(1);
        unsafe {
            let p = CreatePen(PS_SOLID, w, cref(color));
            let old_p = SelectObject(self.d, p.into());
            let dy = if down { hh } else { -hh };
            let _ = MoveToEx(self.d, x - hh, y - dy, None);
            let _ = LineTo(self.d, x, y + dy);
            let _ = LineTo(self.d, x + hh, y - dy);
            SelectObject(self.d, old_p);
            let _ = DeleteObject(p.into());
        }
    }

    fn plus_minus(&self, cx: f32, cy: f32, half: f32, collapsed: bool, color: u32) {
        let w = self.px(1.7).max(1);
        unsafe {
            let p = CreatePen(PS_SOLID, w, cref(color));
            let old_p = SelectObject(self.d, p.into());
            let _ = MoveToEx(self.d, self.px(cx - half), self.px(cy), None);
            let _ = LineTo(self.d, self.px(cx + half), self.px(cy));
            if collapsed {
                let _ = MoveToEx(self.d, self.px(cx), self.px(cy - half), None);
                let _ = LineTo(self.d, self.px(cx), self.px(cy + half));
            }
            SelectObject(self.d, old_p);
            let _ = DeleteObject(p.into());
        }
    }

    fn sun(&self, cx: f32, cy: f32, rad: f32, color: u32) {
        let w = (rad * 0.16).max(1.2);
        self.ring(cx, cy, rad * 0.46, w, color);
        let inner = rad * 0.74;
        let outer = rad * 1.0;
        let wpx = self.px(w).max(1);
        unsafe {
            let p = CreatePen(PS_SOLID, wpx, cref(color));
            let old_p = SelectObject(self.d, p.into());
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (s, c) = a.sin_cos();
                let x1 = self.px(cx + c * inner);
                let y1 = self.px(cy + s * inner);
                let x2 = self.px(cx + c * outer);
                let y2 = self.px(cy + s * outer);
                let _ = windows::Win32::Graphics::Gdi::MoveToEx(self.d, x1, y1, None);
                let _ = windows::Win32::Graphics::Gdi::LineTo(self.d, x2, y2);
            }
            SelectObject(self.d, old_p);
            let _ = DeleteObject(p.into());
        }
    }

    fn text(&mut self, rect: RECT, s: &str, size: f32, color: u32, fmt: u32, bold: bool) {
        let s = i18n::tr(self.lang, s);
        if s.is_empty() {
            return;
        }
        let f = self.font(size, bold);
        let mut buf: Vec<u16> = s.encode_utf16().collect();
        let mut rc = rect;
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

    fn text_w(&mut self, s: &str, size: f32, bold: bool) -> i32 {
        let s = i18n::tr(self.lang, s);
        if s.is_empty() {
            return 0;
        }
        let f = self.font(size, bold);
        let buf: Vec<u16> = s.encode_utf16().collect();
        let mut sz = SIZE::default();
        unsafe {
            let old_f = SelectObject(self.d, f.into());
            let _ = GetTextExtentPoint32W(self.d, &buf, &mut sz);
            SelectObject(self.d, old_f);
        }
        sz.cx
    }
}

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
            let a = start + i as f32 * half / seg as f32;
            pts.push(POINT {
                x: cx + (a.cos() * rad as f32) as i32,
                y: cy + (a.sin() * rad as f32) as i32,
            });
        }
    }
    pts
}

pub struct Gui {
    core: Arc<Core>,
    cfg: Config,
    rev: u64,
    dark: bool,
    hwnd: HWND,
    draw: Draw,
    mem: Option<(HDC, isize, i32, i32)>,
    hits: Vec<(Id, RECT)>,
    hover: Option<Id>,
    pressed: Option<Id>,
    focus: Option<Id>,
    num: String,
    num_up: String,
    num_dn: String,
    editing: bool,
    select_all: bool,
    caret: bool,
    blink_at: Instant,
    tick: u32,
    edit_kind: u8,
    content_h: i32,
    scroll: i32,
    max_scroll: i32,
    tray: Option<TrayIcon>,
    tray_dark: Option<bool>,
    monitors: Vec<winapi_util::Monitor>,
    slider_rect: RECT,
    auto_h: bool,
    set_h: i32,
    lang_open: bool,
    lang_rect: RECT,
    lang_scroll: f32,
    lang_max_scroll: f32,
    capture: Option<u8>,
    mon_capture: Option<(usize, usize)>,
    mon_step_edit: Option<(usize, usize)>,
    mon_step_text: String,
    mon_open: Vec<bool>,
    mon_menu: Option<(usize, usize)>,
    mon_menu_rect: RECT,
}

impl Gui {
    fn new(core: Arc<Core>, for_dark: bool, tray: Option<TrayIcon>, s: f32) -> Self {
        let (cfg, rev) = core.snapshot();
        let lang = cfg.lang;
        Gui {
            core,
            num: format!("{}", cfg.opacity.round() as i32),
            num_up: fmt_step(cfg.step_up),
            num_dn: fmt_step(cfg.step_down),
            cfg,
            rev,
            dark: for_dark,
            hwnd: HWND::default(),
            draw: Draw {
                d: HDC::default(),
                s,
                w: W,
                lang,
                fonts: HashMap::new(),
            },
            mem: None,
            hits: Vec::new(),
            hover: None,
            pressed: None,
            focus: None,
            editing: false,
            select_all: false,
            caret: false,
            blink_at: Instant::now(),
            tick: 0,
            edit_kind: 0,
            content_h: 0,
            scroll: 0,
            max_scroll: 0,
            tray,
            tray_dark: Some(for_dark),
            monitors: winapi_util::monitors(),
            slider_rect: RECT::default(),
            auto_h: true,
            set_h: 0,
            lang_open: false,
            lang_rect: RECT::default(),
            lang_scroll: 0.0,
            lang_max_scroll: 0.0,
            capture: None,
            mon_capture: None,
            mon_step_edit: None,
            mon_step_text: String::new(),
            mon_open: Vec::new(),
            mon_menu: None,
            mon_menu_rect: RECT::default(),
        }
    }

    fn s(&self) -> f32 {
        self.draw.s
    }

    fn w(&self) -> f32 {
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut rc);
        }
        ((rc.right.max(1) as f32) / self.draw.s).max(360.0)
    }

    fn h(&self) -> f32 {
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut rc);
        }
        ((rc.bottom.max(1) as f32) / self.draw.s).max(240.0)
    }

    fn level(&self) -> f32 {
        self.cfg.opacity
    }

    fn push(&mut self, f: impl FnOnce(&mut Config)) {
        self.core.update(f);
        let (cfg, rev) = self.core.snapshot();
        self.cfg = cfg;
        self.rev = rev;
        self.apply_theme();
    }

    fn apply_theme(&mut self) {
        let dark = match self.cfg.theme {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => winapi_util::system_prefers_dark(),
        };
        if dark != self.dark {
            self.dark = dark;
            if let Some(tray_icon) = self.tray.as_ref() {
                if let Some(ic) = icon::tray_icon(dark) {
                    let _ = tray_icon.set_icon(Some(ic));
                }
            }
        }
        if self.tray_dark != Some(dark) {
            self.tray_dark = Some(dark);
            crate::osd::set_dark(dark);
        }
        if !self.hwnd.0.is_null() {
            let v: i32 = if dark { 1 } else { 0 };
            unsafe {
                let _ = DwmSetWindowAttribute(
                    self.hwnd,
                    DWMWINDOWATTRIBUTE(20),
                    &v as *const i32 as *const c_void,
                    4,
                );
                set_window_icons(self.hwnd, dark);
            }
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<Id> {
        self.hits
            .iter()
            .rev()
            .find(|(_, r)| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
            .map(|(id, _)| *id)
    }

    fn repaint(&self) {
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = InvalidateRect(Some(self.hwnd), None, false);
            }
        }
    }

    fn slider_value(&self, x: i32) -> f32 {
        let w = (self.slider_rect.right - self.slider_rect.left).max(1);
        let f = ((x - self.slider_rect.left) as f32 / w as f32).clamp(0.0, 1.0);
        (f * MAX_LEVEL * 10.0).round() / 10.0
    }

    fn buf_get(&self) -> String {
        match self.edit_kind {
            1 => self.num_up.clone(),
            2 => self.num_dn.clone(),
            _ => self.num.clone(),
        }
    }

    fn buf_set(&mut self, text: String) {
        match self.edit_kind {
            1 => self.num_up = text,
            2 => self.num_dn = text,
            _ => self.num = text,
        }
    }

    fn commit_mon_step(&mut self) {
        let Some((i, k)) = self.mon_step_edit.take() else {
            return;
        };
        let text = self.mon_step_text.trim().to_string();
        if let Ok(v) = text.parse::<f32>() {
            let v = v.clamp(0.5, 30.0);
            if let Some(mm) = crate::winapi_util::monitors().get(i) {
                let dev = mm.device.clone();
                let mut list = self.cfg.mon_entries(&dev);
                if let Some(e) = list.get_mut(k) {
                    e.step = v;
                }
                self.cfg.set_mon_entries(&dev, list.clone());
                self.core.update(move |c| c.set_mon_entries(&dev, list));
            }
        }
        self.mon_step_text.clear();
        self.repaint();
    }

    fn commit_input(&mut self) {
        if !self.editing {
            return;
        }
        let text = self.buf_get().trim().to_string();
        if let Ok(v) = text.parse::<f32>() {
            match self.edit_kind {
                1 => {
                    let v = v.clamp(0.0, 100.0);
                    self.cfg.step_up = v;
                    self.core.update(|c| c.step_up = v);
                }
                2 => {
                    let v = v.clamp(0.0, 100.0);
                    self.cfg.step_down = v;
                    self.core.update(|c| c.step_down = v);
                }
                _ => {
                    let v = v.clamp(0.0, MAX_LEVEL);
                    self.push(|c| c.opacity = v);
                }
            }
        }
        self.editing = false;
        self.select_all = false;
        self.edit_kind = 0;
        self.num = format!("{}", self.cfg.opacity.round() as i32);
        self.num_up = fmt_step(self.cfg.step_up);
        self.num_dn = fmt_step(self.cfg.step_down);
        self.repaint();
    }

    fn set_level(&mut self, v: f32) {
        self.push(|c| c.opacity = v.clamp(0.0, MAX_LEVEL));
        if !self.editing {
            self.num = format!("{}", self.cfg.opacity.round() as i32);
        }
        self.repaint();
    }
}

fn fmt_step(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i32)
    } else {
        format!("{:.1}", v)
    }
}

fn paint_content(g: &mut Gui, hdc: HDC) {
    let pc = pal(g.dark);
    g.hits.clear();
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(g.hwnd, &mut rc);
    }
    let view_h = (rc.bottom.max(1)) as f32 / g.s();
    {
        let mut d = Draw {
            d: hdc,
            s: g.draw.s,
            w: g.w(),
            lang: g.cfg.lang,
            fonts: std::mem::take(&mut g.draw.fonts),
        };
        let client = RECT {
            left: 0,
            top: 0,
            right: g.r(0.0, 0.0, d.w, 0.0).right,
            bottom: g.r(0.0, 0.0, d.w, 0.0).right * 4,
        };
        d.fill(client, pc.bg);
        unsafe {
            SetViewportOrgEx(hdc, 0, -g.scroll, None);
        }

        let mut y = 20.0_f32;
        y = header(&mut d, &pc, g, y);
        y = card_dim(&mut d, &pc, g, y);
        y = card_displays(&mut d, &pc, g, y);
        y = card_appearance(&mut d, &pc, g, y);
        y = card_behavior(&mut d, &pc, g, y);
        y = card_keys(&mut d, &pc, g, y);
        y = card_monkeys(&mut d, &pc, g, y);
        y = card_overlay(&mut d, &pc, g, y);
        y = footer(&mut d, &pc, g, y);

        y += 20.0;

        g.content_h = d.px(y);
        if g.lang_open {
            lang_menu(&mut d, &pc, g);
        }
        mon_menu_popup(&mut d, &pc, g);
        unsafe {
            SetViewportOrgEx(hdc, 0, 0, None);
        }
        if g.max_scroll > 0 {
            let x1 = d.w - 9.0;
            let track = d.r(x1, 6.0, 4.0, (view_h - 12.0).max(20.0));
            d.round(track, 2.0, pc.track);
            let th = ((track.bottom - track.top) as f32 * view_h / (g.content_h as f32 / g.s()))
                .max(28.0 * g.s());
            let th = th.min((track.bottom - track.top) as f32);
            let frac = g.scroll as f32 / g.max_scroll as f32;
            let top = track.top as f32 + ((track.bottom - track.top) as f32 - th) * frac;
            let thumb = RECT {
                left: track.left,
                top: top as i32,
                right: track.right,
                bottom: top as i32 + th as i32,
            };
            d.round(thumb, 2.0, pc.dim);
        }
        g.draw.fonts = std::mem::take(&mut d.fonts);
    }
}

impl Gui {
    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> RECT {
        RECT {
            left: (x * self.s()).round() as i32,
            top: (y * self.s()).round() as i32,
            right: ((x + w) * self.s()).round() as i32,
            bottom: ((y + h) * self.s()).round() as i32,
        }
    }
}

fn record(g: &mut Gui, id: Id, r: RECT) {
    let s = g.scroll;
    g.hits.push((
        id,
        RECT {
            top: r.top - s,
            bottom: r.bottom - s,
            ..r
        },
    ));
}

fn set_window_icons(hwnd: HWND, dark: bool) {
    unsafe {
        let sm = GetSystemMetrics(SM_CXSMICON).max(16);
        let lg = GetSystemMetrics(SM_CXICON).max(32);
        let big = crate::icon::hicon(dark, lg);
        let small = crate::icon::hicon(dark, sm);
        if small.0.is_null() || big.0.is_null() {
            return;
        }
        for (kind, h) in [(ICON_BIG, big), (ICON_SMALL, small), (ICON_SMALL2, small)] {
            let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(hwnd, WM_SETICON, Some(WPARAM(kind as usize)), Some(LPARAM(h.0 as isize)));
        }
        SetClassLongPtrW(hwnd, GCLP_HICON, big.0 as isize);
        SetClassLongPtrW(hwnd, GCLP_HICONSM, small.0 as isize);
    }
}

fn header(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32) -> f32 {
    d.sun(PAD + 12.0, y + 20.0, 13.0, pc.accent);
    let brand = crate::i18n::app_name(d.lang);
    let ver = format!("v{}", env!("CARGO_PKG_VERSION"));
    let avail = (d.w - PAD - 246.0 - (PAD + 34.0) - 52.0).max(60.0);
    let mut bsize = 17.0f32;
    while bsize > 11.0 && (d.text_w(brand, bsize, true) as f32 / d.s) > avail {
        bsize -= 1.0;
    }
    d.text(d.r(PAD + 34.0, y + 6.0, avail + 6.0, 24.0), brand, bsize, pc.text, FMT_LEFT | 0x8000, true);
    let bw = d.text_w(brand, bsize, true) as f32 / d.s;
    let vsize = bsize * 0.66;
    d.text(
        d.r(PAD + 34.0 + bw + 4.0, y + 6.0 + (bsize - vsize) + 1.0, 60.0, 18.0),
        &ver,
        vsize,
        pc.dim,
        FMT_LEFT,
        false,
    );
    d.text(
        d.r(PAD + 34.0, y + 30.0, 260.0, 16.0),
        "柔光护眼 · 屏幕亮度调节",
        10.6,
        pc.dim,
        FMT_LEFT,
        false,
    );
    let pct = (100.0 - g.level() / MAX_LEVEL * 100.0).round() as i32;
    let label = format!("亮度 ≈ {pct}%");
    let tw = d.text_w(&label, 10.4, false);
    let w = (tw as f32 / d.s) + 20.0;
    let r = d.r(d.w - PAD - w, y + 8.0, w, 22.0);
    d.round(r, 11.0, pc.track);
    d.text(r, &label, 10.4, pc.text, FMT_CENTER, false);

    let lw = 124.0;
    let lr = d.r(d.w - PAD - w - 8.0 - lw, y + 8.0, lw, 22.0);
    let lr = RECT {
        left: lr.left.max(d.px(PAD + 232.0)),
        ..lr
    };
    d.round(lr, 11.0, pc.track);
    if g.lang_open {
        d.stroke(lr, 11.0, pc.accent, 1.0);
    }
    d.text(
        RECT {
            left: lr.left + d.px(10.0),
            right: lr.right - d.px(10.0),
            ..lr
        },
        LANGS[g.cfg.lang.idx()].name,
        10.0,
        pc.text,
        FMT_LEFT | 0x8000,
        false,
    );
    g.lang_rect = lr;
    record(g, Id::Lang, lr);
    y + 52.0
}

fn lang_menu(d: &mut Draw, pc: &Pal, g: &mut Gui) {
    let n = LANGS.len();
    let ch = 24.0f32;
    let py = 6.0f32;
    let w = 208.0f32;
    let max_h = ch * 8.0 + 2.0 * py;
    let content = ch * n as f32;
    let h = (content + 2.0 * py).min(max_h);
    let bx = (g.lang_rect.right as f32 / d.s - w)
        .max(PAD)
        .min((d.w - PAD - w).max(PAD));
    let mut by = g.lang_rect.bottom as f32 / d.s + 6.0;
    let ch_client = g.h();
    if by + h > ch_client - 12.0 {
        by = (ch_client - 12.0 - h).max(8.0);
    }
    let box_r = d.r(bx, by, w, h);
    d.round(box_r, 10.0, pc.card);
    d.stroke(box_r, 10.0, pc.stroke, 1.0);

    let view_h = h - 2.0 * py;
    let max_scroll = (content - view_h).max(0.0);
    g.lang_max_scroll = max_scroll;
    g.lang_scroll = g.lang_scroll.clamp(0.0, max_scroll);
    let ls = g.lang_scroll;
    let top0 = by + py;
    let bot0 = top0 + view_h;
    for i in 0..n {
        let top = top0 + i as f32 * ch - ls;
        if top < top0 - 0.5 || top + ch > bot0 + 0.5 {
            continue;
        }
        let cell = RECT {
            left: d.px(bx + 2.0),
            top: d.px(top),
            right: d.px(bx + w - 2.0),
            bottom: d.px(top + ch),
        };
        let cur = g.cfg.lang.idx() == i;
        if g.hover == Some(Id::LangItem(i)) || cur {
            d.round(cell, 7.0, pc.track);
        }
        if cur {
            d.circle(
                bx + 11.0,
                top + ch / 2.0,
                2.4,
                pc.accent,
            );
        }
        d.text(
            RECT { left: cell.left + d.px(18.0), ..cell },
            LANGS[i].name,
            10.0,
            if cur { pc.accent } else { pc.text },
            FMT_LEFT | 0x8000,
            cur,
        );
        record(g, Id::LangItem(i), cell);
    }
    if max_scroll > 0.0 {
        let tx = bx + w - 7.0;
        d.round(d.r(tx, top0 + 2.0, 3.0, view_h - 4.0), 1.5, pc.track);
        let th = (view_h * view_h / content).max(20.0);
        let ty = top0 + (view_h - th) * (ls / max_scroll);
        d.round(d.r(tx, ty, 3.0, th), 1.5, pc.dim);
    }
}

fn card_base(d: &mut Draw, pc: &Pal, y: f32, h: f32) -> (f32, f32) {
    let r = d.r(PAD, y, d.w - 2.0 * PAD, h);
    d.round(r, 14.0, pc.card);
    d.stroke(r, 14.0, pc.stroke, 1.0);
    (r.left as f32 / d.s, y)
}

fn card_dim(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32) -> f32 {
    let h = 200.0;
    card_base(d, pc, y, h);
    let x0 = PAD;
    let x1 = d.w - PAD;
    d.text(d.r(x0 + 16.0, y + 14.0, 200.0, 20.0), "亮度不透明度", 12.4, pc.text, FMT_LEFT, true);
    d.text(
        d.r(x1 - 260.0, y + 15.0, 244.0, 20.0),
        "0% = 透明 · 90% = 最暗",
        10.2,
        pc.dim,
        FMT_RIGHT,
        false,
    );

    let shown = if g.editing {
        g.num.clone()
    } else {
        format!("{}", g.level().round() as i32)
    };
    let num_r = d.r(x0 + 16.0, y + 38.0, 150.0, 50.0);
    d.text(num_r, &shown, 34.0, pc.text, FMT_LEFT, true);
    let nw = d.text_w(&shown, 34.0, true);
    d.text(
        d.r(x0 + 16.0 + nw as f32 / d.s + 5.0, y + 62.0, 30.0, 22.0),
        "%",
        12.0,
        pc.dim,
        FMT_LEFT,
        false,
    );

    let lbl = "自定义";
    let lw = d.text_w(lbl, 10.2, false);
    let box_w = 46.0;
    let pct_w = d.text_w("%", 10.2, false);
    let total = lw as f32 / d.s + 8.0 + box_w + 5.0 + pct_w as f32 / d.s;
    let start = x1 - 16.0 - total;
    d.text(
        d.r(start, y + 54.0, lw as f32 / d.s + 2.0, 20.0),
        lbl,
        10.2,
        pc.dim,
        FMT_LEFT,
        false,
    );
    let input_r = d.r(start + lw as f32 / d.s + 8.0, y + 48.0, box_w, 26.0);
    let act = g.editing && g.edit_kind == 0;
    if act {
        d.round(input_r, 7.0, pc.track);
        d.stroke(input_r, 7.0, pc.accent, 1.4);
    } else {
        d.round(input_r, 7.0, pc.track);
    }
    let carets = if act && g.caret { "|" } else { "" };
    let itext = if act {
        format!("{}{}", if g.select_all { "" } else { "" }, g.num)
    } else {
        format!("{}", g.level().round() as i32)
    };
    d.text(input_r, &itext, 12.0, pc.text, FMT_CENTER, false);
    if act && g.caret {
        let tw = d.text_w(&itext, 12.0, false);
        let cx = (input_r.left + input_r.right) / 2 + tw / 2 + d.px(2.0);
        d.fill(
            RECT {
                left: cx,
                top: input_r.top + d.px(6.0),
                right: cx + d.px(1.2).max(1),
                bottom: input_r.bottom - d.px(6.0),
            },
            pc.accent,
        );
    }
    let _ = carets;
    d.text(
        d.r(start + lw as f32 / d.s + 8.0 + box_w + 5.0, y + 54.0, 20.0, 20.0),
        "%",
        10.2,
        pc.dim,
        FMT_LEFT,
        false,
    );
    record(g, Id::Input, input_r);

    let track = d.r(x0 + 16.0, y + 116.0, d.w - 2.0 * PAD - 32.0, 7.0);
    g.slider_rect = track;
    let track_hit = RECT {
        left: track.left,
        top: track.top - d.px(14.0),
        right: track.right,
        bottom: track.bottom + d.px(14.0),
    };
    d.round(track, 4.0, pc.track);
    let frac = (g.level() / MAX_LEVEL).clamp(0.0, 1.0);
    let filled = RECT {
        left: track.left,
        top: track.top,
        right: track.left + (d.px(frac) as f32 * (track.right - track.left) as f32 / d.px(1.0) as f32) as i32,
        bottom: track.bottom,
    };
    let filled = RECT {
        right: track.left + (((track.right - track.left) as f32) * frac) as i32,
        ..filled
    };
    if filled.right > filled.left {
        d.round(filled, 4.0, pc.accent);
    }
    let cx = (track.left as f32 + (track.right - track.left) as f32 * frac) / d.s;
    let cy = (track.top + track.bottom) as f32 / 2.0 / d.s;
    d.circle(cx, cy, 9.0, pc.knob);
    d.circle(cx, cy, 6.0, pc.accent);
    record(g, Id::Slider, track_hit);

    d.text(
        d.r(x0 + 16.0, y + 138.0, d.w - 2.0 * PAD - 32.0, 16.0),
        "拖动或点击滑块；Tab 聚焦后用 ← → 微调（Shift 加速）",
        9.9,
        pc.dim,
        FMT_LEFT,
        false,
    );

    let mut cx = x0 + 16.0;
    for (i, (label, value)) in CHIPS.iter().enumerate() {
        let tw = d.text_w(label, 10.4, false) as f32 / d.s;
        let cw = tw + 24.0;
        let r = d.r(cx, y + 158.0, cw, 26.0);
        let selected = (g.level() - *value).abs() < 0.6;
        d.round(r, 8.0, if selected { pc.accent } else { pc.track });
        d.text(
            r,
            label,
            10.4,
            if selected { pc.knob } else { pc.text },
            FMT_CENTER,
            false,
        );
        record(g, Id::Chip(i), r);
        cx += cw + 8.0;
    }
    y + h + 12.0
}

fn card_displays(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32) -> f32 {
    let n = g.monitors.len().max(1);
    let h = 46.0 + n as f32 * 40.0 + 10.0;
    card_base(d, pc, y, h);
    let x0 = PAD + 16.0;
    d.text(d.r(x0, y + 14.0, 120.0, 20.0), "显示器", 12.4, pc.text, FMT_LEFT, true);
    d.text(
        d.r(d.w - PAD - 290.0, y + 15.0, 274.0, 20.0),
        "点掉某个显示器，就不再为它加遮罩",
        10.2,
        pc.dim,
        FMT_RIGHT,
        false,
    );
    if g.monitors.is_empty() {
        d.text(d.r(x0, y + 40.0, 300.0, 20.0), "未检测到显示器", 11.0, pc.dim, FMT_LEFT, false);
        return y + h + 12.0;
    }
    let monitors = g.monitors.clone();
    for (i, m) in monitors.iter().enumerate() {
        let ry = y + 44.0 + i as f32 * 40.0;
        let on = g.cfg.display_on(&m.device);
        let title = mon_name(d.lang, i);
        let rowc = ry + 19.5;
        let rowt = rowc - 12.5 + 2.2;
        d.text(d.r(x0, rowt, 140.0, 25.0), &title, 11.6, pc.text, FMT_LEFT, false);
        let tw = d.text_w(&title, 11.6, false) as f32 / d.s;
        let name_right = d.w - PAD - 16.0 - 42.0 - 16.0;
        let name_x = x0 + tw + 10.0;
        let mut info_x = name_x;
        let friendly = m.friendly.trim();
        if !friendly.is_empty() {
            let room = (name_right - name_x - 96.0).max(40.0);
            d.text(
                d.r(name_x, rowt, room, 25.0),
                friendly,
                11.6,
                pc.text,
                FMT_LEFT | 0x8000,
                false,
            );
            info_x = name_x + room + 10.0;
        }
        let info = format!("{}×{}", m.width(), m.height());
        d.text(d.r(info_x, rowt + 1.2, 120.0, 22.0), &info, 10.0, pc.dim, FMT_LEFT, false);
        let iw = d.text_w(&info, 10.0, false) as f32 / d.s;
        if m.primary {
            let pw = d.text_w("主屏", 9.4, false) as f32 / d.s + 14.0;
            let pr = d.r(info_x + iw + 8.0, rowc - 9.0 + 1.6, pw, 18.0);
            d.round(pr, 6.0, pc.track);
            d.text(pr, "主屏", 9.4, pc.dim, FMT_CENTER, false);
        }
        let w = d.px(42.0);
        let hh = d.px(23.0);
        let top = d.px(rowc - 11.5);
        let rect = RECT {
            left: d.px(d.w - PAD - 16.0) - w,
            top,
            right: d.px(d.w - PAD - 16.0),
            bottom: top + hh,
        };
        d.round(rect, (hh / 2) as f32, if on { pc.accent } else { pc.track });
        let kr = 8.6;
        let kx = if on {
            (rect.right - d.px(3.4)) as f32 / d.s - kr
        } else {
            (rect.left + d.px(3.4)) as f32 / d.s + kr
        };
        d.circle(kx, (rect.top + rect.bottom) as f32 / d.s / 2.0, kr, pc.knob);
        record(
            g,
            Id::Display(i),
            RECT {
                left: rect.left - d.px(8.0),
                top: top - d.px(8.0),
                right: rect.right + d.px(8.0),
                bottom: rect.bottom + d.px(8.0),
            },
        );
    }
    y + h + 12.0
}

fn card_appearance(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32) -> f32 {
    let h = 116.0;
    card_base(d, pc, y, h);
    let x0 = PAD + 16.0;
    d.text(d.r(x0, y + 12.0, 200.0, 18.0), "外观", 10.0, pc.dim, FMT_LEFT, false);
    d.text(d.r(x0, y + 30.0, 200.0, 20.0), "主题", 12.2, pc.text, FMT_LEFT, false);
    d.text(
        d.r(x0, y + 50.0, 320.0, 18.0),
        "跟随系统时自动匹配 Windows 明暗主题",
        9.9,
        pc.dim,
        FMT_LEFT,
        false,
    );
    let seg = d.r(x0, y + 74.0, d.w - 2.0 * PAD - 32.0, 28.0);
    d.round(seg, 9.0, pc.track);
    let modes = [ThemeMode::System, ThemeMode::Dark, ThemeMode::Light];
    let cw = (seg.right - seg.left) / 3;
    for (i, m) in modes.iter().enumerate() {
        let r = RECT {
            left: seg.left + cw * i as i32 + d.px(2.0),
            top: seg.top + d.px(2.0),
            right: seg.left + cw * (i as i32 + 1) - d.px(2.0),
            bottom: seg.bottom - d.px(2.0),
        };
        let selected = g.cfg.theme == *m;
        if selected {
            d.round(r, 8.0, pc.accent);
        }
        d.text(
            r,
            m.label(),
            10.8,
            if selected { pc.knob } else { pc.text },
            FMT_CENTER,
            false,
        );
        record(g, Id::Seg(i), r);
    }
    y + h + 12.0
}

const MON_ACT_LABELS: [&str; 3] = ["加深", "减淡", "关闭"];

fn mon_name(lang: Lang, i: usize) -> String {
    crate::i18n::tr(lang, &format!("显示器 {}", i + 1))
}

fn glob_label(lang: Lang, act: &str) -> String {
    format!(
        "{} · {}",
        crate::i18n::ui(lang, "全局快捷键"),
        crate::i18n::ui(lang, act)
    )
}

fn mon_label(lang: Lang, i: usize, k: usize, act: &str) -> String {
    format!("{} {} · {}", mon_name(lang, i), k + 1, crate::i18n::ui(lang, act))
}

fn glob_labels(lang: Lang) -> [String; 4] {
    [
        glob_label(lang, "加深"),
        glob_label(lang, "减淡"),
        glob_label(lang, "关闭"),
        format!(
            "{} · {}",
            crate::i18n::ui(lang, "全局快捷键"),
            crate::i18n::tr(lang, "显示主界面")
        ),
    ]
}

fn all_keys(cfg: &Config, lang: Lang) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    let labs = glob_labels(lang);
    let glob: [(String, String); 4] = [
        (labs[0].clone(), cfg.hk_inc.clone()),
        (labs[1].clone(), cfg.hk_dec.clone()),
        (labs[2].clone(), cfg.hk_off.clone()),
        (labs[3].clone(), cfg.hk_toggle.clone()),
    ];
    for (lab, spec) in glob.into_iter() {
        if !spec.is_empty() {
            v.push((spec, lab));
        }
    }
    for (i, m) in crate::winapi_util::monitors().iter().enumerate() {
        for (k, e) in cfg.mon_entries(&m.device).iter().enumerate() {
            if e.spec.is_empty() {
                continue;
            }
            let act = MON_ACT_LABELS[e.act.min(2) as usize];
            v.push((e.spec.clone(), mon_label(lang, i, k, act)));
        }
    }
    v
}

fn conflict_with(own: &str, label: &str, all: &[(String, String)], lang: Lang) -> Option<String> {
    if own.is_empty() {
        return None;
    }
    let mut names: Vec<String> = Vec::new();
    for (spec, lab) in all {
        if spec.eq_ignore_ascii_case(own) && lab != label && !names.iter().any(|n| n == lab) {
            names.push(lab.clone());
        }
    }
    if names.is_empty() {
        None
    } else {
        Some(format!("{}: {}", crate::i18n::ui(lang, "快捷键冲突"), names.join(" / ")))
    }
}

fn key_chip(d: &mut Draw, pc: &Pal, g: &mut Gui, rect: RECT, id: Id, spec: &str, active: bool, bad: bool) {
    let hot = active || g.hover == Some(id);
    d.round(rect, 8.0, pc.track);
    if bad {
        d.stroke(rect, 8.0, 0xE5484D, 1.6);
    } else if hot {
        d.stroke(rect, 8.0, if active { pc.accent } else { pc.stroke }, 1.4);
    }
    let shown = if active {
        "…  Esc".to_string()
    } else if spec.trim().is_empty() {
        "—".to_string()
    } else {
        spec.to_string()
    };
    let col = if active {
        pc.accent
    } else if bad {
        0xE5484D
    } else {
        pc.dim
    };
    d.text(
        RECT {
            right: rect.right - d.px(8.0),
            ..rect
        },
        &shown,
        10.0,
        col,
        FMT_RIGHT,
        false,
    );
    record(g, id, rect);
}

fn card_keys(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32) -> f32 {
    let all = all_keys(&g.cfg, d.lang);
    let labs = glob_labels(d.lang);
    let rows: [(u8, String, String, String); 4] = [
        (0, crate::i18n::ui(d.lang, "加深"), g.cfg.hk_inc.clone(), labs[0].clone()),
        (1, crate::i18n::ui(d.lang, "减淡"), g.cfg.hk_dec.clone(), labs[1].clone()),
        (2, crate::i18n::ui(d.lang, "关闭"), g.cfg.hk_off.clone(), labs[2].clone()),
        (
            3,
            crate::i18n::tr(d.lang, "显示主界面"),
            g.cfg.hk_toggle.clone(),
            labs[3].clone(),
        ),
    ];
    let notes: Vec<Option<String>> = rows
        .iter()
        .map(|(_, _, spec, lab)| conflict_with(spec, lab, &all, d.lang))
        .collect();
    let note_h: f32 = notes.iter().map(|n| if n.is_some() { 16.0 } else { 0.0 }).sum();
    let h = 30.0 + 44.0 + 4.0 * 34.0 + note_h + 2.0 * 36.0 + 18.0;
    card_base(d, pc, y, h);
    let x0 = PAD + 16.0;
    let right = d.w - PAD - 16.0;
    row_toggle(d, pc, g, y + 18.0, Id::Toggle(T_HOTKEY), "全局快捷键", "", g.cfg.hotkeys);
    let mut ry = y + 18.0 + 44.0 + 4.0;
    for (i, (k, label, spec, _lab)) in rows.iter().enumerate() {
        let rect = d.r(x0, ry, right - x0, 30.0);
        let id = Id::HkChip(*k);
        let active = g.capture == Some(*k);
        d.round(rect, 8.0, pc.card);
        let bad = notes[i].is_some();
        if bad {
            d.stroke(rect, 8.0, 0xE5484D, 1.4);
        } else if active || g.hover == Some(id) {
            d.stroke(rect, 8.0, if active { pc.accent } else { pc.stroke }, 1.4);
        }
        d.text(d.r(x0 + 10.0, ry + 7.0, 170.0, 18.0), label, 10.5, pc.text, FMT_LEFT, false);
        let shown = if active { "…  Esc".to_string() } else { spec.clone() };
        let col = if active {
            pc.accent
        } else if bad {
            0xE5484D
        } else {
            pc.dim
        };
        d.text(
            d.r(right - 200.0, ry + 7.0, 190.0, 18.0),
            &shown,
            10.5,
            col,
            FMT_RIGHT,
            false,
        );
        record(g, id, rect);
        ry += 34.0;
        if let Some(n) = &notes[i] {
            d.text(d.r(x0 + 10.0, ry - 3.0, right - x0 - 20.0, 16.0), n, 9.0, 0xE5484D, FMT_LEFT, false);
            ry += 16.0;
        }
    }
    let srow = |d: &mut Draw, pc: &Pal, g: &mut Gui, base_y: f32, label: &str, cur: f32, up: bool| {
        let fields: [f32; 4] = [1.0, 2.0, 5.0, 10.0];
        let full = crate::i18n::tr(d.lang, label);
        let short = full.split("  ").next().unwrap_or(&full).trim().to_string();
        let lab = d.r(x0 + 2.0, base_y + 7.0, 92.0, 18.0);
        d.text(lab, &short, 10.5, pc.text, FMT_LEFT, false);
        let kind: u8 = if up { 1 } else { 2 };
        let inw = 42.0;
        let pct_w = 12.0;
        let chips_right = right - inw - pct_w - 14.0;
        let cx0 = x0 + 96.0;
        let gap = 5.0;
        let cw = ((chips_right - cx0) - 3.0 * gap) / 4.0;
        for (i, v) in fields.iter().enumerate() {
            let crect = d.r(cx0 + i as f32 * (cw + gap), base_y, cw, 30.0);
            let id = if up { Id::StepUp(i) } else { Id::StepDown(i) };
            let on = (cur - *v).abs() < 0.01;
            d.round(crect, 8.0, if on { pc.accent } else { pc.card });
            if !on && g.hover == Some(id) {
                d.stroke(crect, 8.0, pc.stroke, 1.4);
            }
            let txt = format!("±{}%", *v as i32);
            let col = if on { 0xFFFFFF } else { pc.dim };
            d.text(crect, &txt, 10.0, col, FMT_CENTER, false);
            record(g, id, crect);
        }
        let inr = d.r(chips_right + 4.0, base_y, inw, 30.0);
        let active = g.editing && g.edit_kind == kind;
        d.round(inr, 8.0, pc.track);
        if active {
            d.stroke(inr, 8.0, pc.accent, 1.4);
        }
        let itext = if active { g.buf_get() } else { fmt_step(cur) };
        d.text(inr, &itext, 10.0, pc.text, FMT_CENTER, false);
        if active && g.caret {
            let tw = d.text_w(&itext, 10.0, false);
            let cx = (inr.left + inr.right) / 2 + tw / 2 + d.px(2.0);
            d.fill(
                RECT {
                    left: cx,
                    top: inr.top + d.px(7.0),
                    right: cx + d.px(1.2).max(1),
                    bottom: inr.bottom - d.px(7.0),
                },
                pc.accent,
            );
        }
        d.text(
            d.r(inr.right as f32 / d.s + 4.0, base_y + 7.0, pct_w, 18.0),
            "%",
            10.0,
            pc.dim,
            FMT_LEFT,
            false,
        );
        record(g, Id::StepIn(kind), inr);
    };
    srow(d, pc, g, ry + 2.0, "遮罩加深  +5%", g.cfg.step_up, true);
    srow(d, pc, g, ry + 38.0, "遮罩减淡  -5%", g.cfg.step_down, false);
    y + h + 12.0
}

fn card_monkeys(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32) -> f32 {
    let mons = crate::winapi_util::monitors();
    if g.mon_open.len() != mons.len() {
        g.mon_open.resize(mons.len(), false);
    }
    let all = all_keys(&g.cfg, d.lang);
    let mut h = 44.0;
    for (i, m) in mons.iter().enumerate() {
        h += 34.0 + 8.0;
        if !g.mon_open.get(i).copied().unwrap_or(false) {
            continue;
        }
        let list = g.cfg.mon_entries(&m.device);
        let mut notes = 0.0;
        for (k, e) in list.iter().enumerate() {
            let act = MON_ACT_LABELS[e.act.min(2) as usize];
            if conflict_with(&e.spec, &mon_label(d.lang, i, k, act), &all, d.lang).is_some() {
                notes += 16.0;
            }
            if e.act != 2 {
                notes += 0.0;
            }
        }
        let step_rows = list.iter().filter(|e| e.act != 2).count() as f32 * 30.0;
        h += 6.0 + list.len() as f32 * 34.0 + notes + step_rows + 34.0;
    }
    let h = h + 12.0;
    card_base(d, pc, y, h);
    let x0 = PAD + 16.0;
    let right = d.w - PAD - 16.0;
    d.text(
        d.r(x0, y + 12.0, 300.0, 18.0),
        &crate::i18n::ui(d.lang, "每屏独立快捷键"),
        10.0,
        pc.dim,
        FMT_LEFT,
        false,
    );
    let mut ry = y + 32.0;
    if mons.is_empty() {
        d.text(
            d.r(x0, ry + 6.0, right - x0, 18.0),
            &crate::i18n::tr(d.lang, "未检测到显示器"),
            10.0,
            pc.dim,
            FMT_LEFT,
            false,
        );
        return y + h + 12.0;
    }
    for (i, m) in mons.iter().enumerate() {
        let open = g.mon_open.get(i).copied().unwrap_or(false);
        let list = g.cfg.mon_entries(&m.device);
        let mut notes = 0.0;
        let mut steps_h = 0.0;
        for (k, e) in list.iter().enumerate() {
            let act = MON_ACT_LABELS[e.act.min(2) as usize];
            if conflict_with(&e.spec, &mon_label(d.lang, i, k, act), &all, d.lang).is_some() {
                notes += 16.0;
            }
            if e.act != 2 {
                steps_h += 30.0;
            }
        }
        let box_h = 34.0
            + if open {
                6.0 + list.len() as f32 * 34.0 + notes + steps_h + 34.0
            } else {
                0.0
            };
        let container = d.r(x0, ry, right - x0, box_h);
        let hid = Id::MonHead(i);
        d.round(container, 10.0, pc.card);
        if g.hover == Some(hid) {
            d.stroke(container, 10.0, pc.stroke, 1.3);
        } else {
            d.stroke(container, 10.0, pc.track, 1.0);
        }
        let label = mon_name(d.lang, i);
        let name = if m.friendly.is_empty() {
            label
        } else {
            format!("{}  {}", label, m.friendly)
        };
        let res = format!("{}×{}", m.width(), m.height());
        d.text(
            d.r(right - 116.0, ry + 8.0, 82.0, 18.0),
            &res,
            9.5,
            pc.dim,
            FMT_RIGHT,
            false,
        );
        d.text(
            d.r(x0 + 12.0, ry + 8.0, right - x0 - 168.0, 18.0),
            &name,
            10.5,
            pc.text,
            FMT_LEFT,
            false,
        );
        d.plus_minus(right - 21.0, ry + 17.0, 4.6, !open, pc.dim);
        record(g, hid, d.r(x0, ry, right - x0, 34.0));
        if !open {
            ry += 34.0 + 8.0;
            continue;
        }
        let mut iy = ry + 34.0 + 6.0;
        for (k, e) in list.iter().enumerate() {
            let act = MON_ACT_LABELS[e.act.min(2) as usize];
            let note = conflict_with(&e.spec, &mon_label(d.lang, i, k, act), &all, d.lang);
            let act_rect = d.r(x0 + 12.0, iy, 70.0, 28.0);
            let aid = Id::MonAct(i, k);
            let menu_open = g.mon_menu == Some((i, k));
            d.round(act_rect, 7.0, if menu_open { pc.accent } else { pc.track });
            d.text(
                act_rect,
                &crate::i18n::ui(d.lang, act),
                9.8,
                if menu_open { 0xFFFFFF } else { pc.text },
                FMT_CENTER,
                false,
            );
            record(g, aid, act_rect);
            if menu_open {
                g.mon_menu_rect = act_rect;
            }
            let spec_rect = d.r(x0 + 88.0, iy, right - x0 - 88.0 - 40.0, 28.0);
            key_chip(
                d,
                pc,
                g,
                spec_rect,
                Id::MonKey(i, k),
                &e.spec,
                g.mon_capture == Some((i, k)),
                note.is_some(),
            );
            let del = d.r(right - 32.0, iy, 24.0, 28.0);
            let did = Id::MonDel(i, k);
            if g.hover == Some(did) {
                d.round(del, 7.0, pc.track);
            }
            d.text(del, "×", 12.0, pc.dim, FMT_CENTER, false);
            record(g, did, del);
            iy += 34.0;
            if e.act != 2 {
                let chips = [1.0f32, 2.0, 5.0, 10.0];
                for (si, v) in chips.iter().enumerate() {
                    let cr = d.r(x0 + 88.0 + si as f32 * 52.0, iy, 46.0, 24.0);
                    let sid = Id::MonStep(i, k, si);
                    let on = (e.step - v).abs() < 0.01;
                    d.round(cr, 6.0, if on { pc.accent } else { pc.track });
                    let t = format!("±{}%", *v as i32);
                    d.text(
                        cr,
                        &t,
                        9.0,
                        if on { 0xFFFFFF } else { pc.text },
                        FMT_CENTER,
                        false,
                    );
                    record(g, sid, cr);
                }
                let sed = Id::MonStepIn(i, k);
                let sed_on = g.mon_step_edit == Some((i, k));
                let inr = d.r(right - 74.0, iy, 46.0, 24.0);
                d.round(inr, 6.0, pc.track);
                if sed_on {
                    d.stroke(inr, 6.0, pc.accent, 1.4);
                }
                let itext = if sed_on {
                    g.mon_step_text.clone()
                } else {
                    fmt_step(e.step)
                };
                d.text(inr, &itext, 9.0, pc.text, FMT_CENTER, false);
                if sed_on && g.caret {
                    let tw = d.text_w(&itext, 9.0, false);
                    let cx = (inr.left + inr.right) / 2 + tw / 2 + d.px(2.0);
                    d.fill(
                        RECT {
                            left: cx,
                            top: inr.top + d.px(6.0),
                            right: cx + d.px(1.2).max(1),
                            bottom: inr.bottom - d.px(6.0),
                        },
                        pc.accent,
                    );
                }
                record(g, sed, inr);
                d.text(
                    d.r(inr.right as f32 / d.s + 3.0, iy + 4.0, 12.0, 16.0),
                    "%",
                    9.0,
                    pc.dim,
                    FMT_LEFT,
                    false,
                );
                iy += 30.0;
            }
            if let Some(n) = &note {
                d.text(d.r(x0 + 88.0, iy - 5.0, right - x0 - 96.0, 16.0), n, 9.0, 0xE5484D, FMT_LEFT, false);
                iy += 16.0;
            }
        }
        let add = d.r(x0 + 12.0, iy, right - x0 - 24.0, 28.0);
        let adid = Id::MonAdd(i);
        d.round(add, 7.0, pc.card);
        d.stroke(add, 7.0, if g.hover == Some(adid) { pc.accent } else { pc.track }, 1.0);
        d.text(
            add,
            &format!("＋ {}", crate::i18n::ui(d.lang, "添加快捷键")),
            9.8,
            pc.accent,
            FMT_CENTER,
            false,
        );
        record(g, adid, add);
        ry += 34.0 + 6.0 + list.len() as f32 * 34.0 + notes + steps_h + 34.0 + 8.0;
    }
    y + h + 12.0
}

fn mon_menu_popup(d: &mut Draw, pc: &Pal, g: &mut Gui) {
    let (mi, mk) = match g.mon_menu {
        Some(v) => v,
        None => return,
    };
    let anchor = g.mon_menu_rect;
    let ch = 26.0;
    let h = ch * 3.0 + 8.0;
    let top = anchor.bottom + d.px(4.0);
    let rect = RECT {
        left: anchor.left,
        top,
        right: anchor.left + d.px(104.0),
        bottom: top + d.px(h),
    };
    d.round(rect, 8.0, pc.card);
    d.stroke(rect, 8.0, pc.stroke, 1.2);
    for m in 0..3u8 {
        let cell = RECT {
            left: rect.left,
            top: rect.top + d.px(4.0 + m as f32 * ch),
            right: rect.right,
            bottom: rect.top + d.px(4.0 + (m as f32 + 1.0) * ch),
        };
        let id = Id::MonActItem(mi, mk, m);
        if g.hover == Some(id) {
            d.round(cell, 6.0, pc.track);
        }
        let cur = crate::winapi_util::monitors()
            .get(mi)
            .map(|mm| {
                g.cfg
                    .mon_entries(&mm.device)
                    .get(mk)
                    .map(|e| e.act)
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let col = if cur == m { pc.accent } else { pc.text };
        d.text(
            RECT {
                left: cell.left + d.px(10.0),
                ..cell
            },
            &crate::i18n::ui(d.lang, MON_ACT_LABELS[m as usize]),
            9.8,
            col,
            FMT_LEFT,
            false,
        );
        record(g, id, cell);
    }
}

fn card_behavior(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32) -> f32 {
    let h = 44.0 + 3.0 * 44.0 + 2.0 * 36.0 + 18.0;
    card_base(d, pc, y, h);
    let x0 = PAD + 16.0;
    let right = d.w - PAD - 16.0;
    d.text(d.r(x0, y + 12.0, 200.0, 18.0), "行为", 10.0, pc.dim, FMT_LEFT, false);
    let rows: [(Id, &str, &str, bool); 3] = [
        (
            Id::Toggle(T_TRAY),
            "关闭窗口时最小化到托盘",
            "关闭后仍在后台运行，从托盘图标恢复",
            g.cfg.close_to_tray,
        ),
        (
            Id::Toggle(T_AUTO),
            "开机自动启动",
            "登录 Windows 后自动运行",
            g.cfg.autostart,
        ),
        (
            Id::Toggle(T_RESTORE),
            "启动时恢复上次亮度",
            "关闭后下次打开还是同样的遮罩强度",
            g.cfg.restore_opacity,
        ),
    ];
    for (i, (id, title, sub, on)) in rows.iter().enumerate() {
        row_toggle(d, pc, g, y + 32.0 + i as f32 * 44.0, *id, title, sub, *on);
    }
    let oy = y + 32.0 + 3.0 * 44.0 + 8.0;
    let olab = crate::i18n::ui(d.lang, "亮度调节行为");
    d.text(d.r(x0 + 2.0, oy + 7.0, 110.0, 18.0), &olab, 10.5, pc.text, FMT_LEFT, false);
    let cx0 = x0 + 120.0;
    let gap = 5.0;
    let cw = ((right - cx0) - 2.0 * gap) / 3.0;
    let modes = [OsdMode::Force, OsdMode::Compat, OsdMode::Off];
    for (i, m) in modes.iter().enumerate() {
        let crect = d.r(cx0 + i as f32 * (cw + gap), oy, cw, 30.0);
        let on = g.cfg.osd == *m;
        let id = Id::Osd(i);
        d.round(crect, 8.0, if on { pc.accent } else { pc.card });
        if !on && g.hover == Some(id) {
            d.stroke(crect, 8.0, pc.stroke, 1.4);
        }
        let label = crate::i18n::ui(d.lang, m.label());
        d.text(crect, &label, 10.0, if on { 0xFFFFFF } else { pc.dim }, FMT_CENTER, false);
        record(g, id, crect);
    }
    y + h + 12.0
}

fn row_toggle(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32, id: Id, title: &str, sub: &str, on: bool) {
    let x0 = PAD + 16.0;
    let right = d.w - PAD - 16.0;
    d.text(d.r(x0, y + 4.0, right - x0 - 60.0, 20.0), title, 12.0, pc.text, FMT_LEFT, false);
    d.text(d.r(x0, y + 23.0, right - x0 - 60.0, 16.0), sub, 9.9, pc.dim, FMT_LEFT, false);
    let w = d.px(42.0);
    let h = d.px(23.0);
    let top = d.px(y + 11.0);
    let rect = RECT {
        left: d.px(right) - w,
        top,
        right: d.px(right),
        bottom: top + h,
    };
    let hit = RECT {
        left: rect.left - d.px(8.0),
        top: top - d.px(10.0),
        right: rect.right + d.px(8.0),
        bottom: rect.bottom + d.px(10.0),
    };
    d.round(rect, (h / 2) as f32, if on { pc.accent } else { pc.track });
    let kr = 8.6;
    let kx = if on {
        (rect.right - d.px(3.4)) as f32 / d.s
    } else {
        (rect.left + d.px(3.4)) as f32 / d.s
    } + if on { -kr } else { kr };
    d.circle(kx, (rect.top + rect.bottom) as f32 / d.s / 2.0, kr, pc.knob);
    record(g, id, hit);
}

fn card_overlay(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32) -> f32 {
    let h = 44.0 + 2.0 * 44.0 + 8.0;
    card_base(d, pc, y, h);
    let x0 = PAD + 16.0;
    d.text(d.r(x0, y + 12.0, 200.0, 18.0), "遮罩", 10.0, pc.dim, FMT_LEFT, false);
    let rows: [(Id, &str, &str, bool); 2] = [
        (
            Id::Toggle(T_PASS),
            "鼠标穿透",
            "遮罩不阻挡鼠标，照常操作下面的窗口",
            g.cfg.passthrough,
        ),
        (
            Id::Toggle(T_TOP),
            "始终置顶",
            "遮罩压在普通窗口之上",
            g.cfg.always_on_top,
        ),
    ];
    for (i, (id, title, sub, on)) in rows.iter().enumerate() {
        row_toggle(d, pc, g, y + 32.0 + i as f32 * 44.0, *id, title, sub, *on);
    }
    y + h + 12.0
}

fn footer(d: &mut Draw, pc: &Pal, g: &mut Gui, y: f32) -> f32 {
    let y = y + 2.0;
    let left = d.r(PAD, y, 118.0, 34.0);
    d.round(left, 10.0, pc.card);
    d.stroke(left, 10.0, pc.stroke, 1.0);
    d.text(left, "恢复默认值", 11.0, pc.text, FMT_CENTER, false);
    record(g, Id::Btn(0), left);

    let right = d.r(d.w - PAD - 96.0, y, 96.0, 34.0);
    d.round(right, 10.0, pc.track);
    d.text(right, "退出程序", 11.0, pc.text, FMT_CENTER, false);
    record(g, Id::Btn(1), right);
    y + 34.0
}

impl Gui {
    fn on_click(&mut self, id: Id) {
        match id {
            Id::Chip(i) => self.set_level(CHIPS[i].1),
            Id::Seg(i) => {
                let mode = [ThemeMode::System, ThemeMode::Dark, ThemeMode::Light][i];
                self.push(|c| c.theme = mode);
            }
            Id::Toggle(k) => match k {
                T_TRAY => self.push(|c| c.close_to_tray = !c.close_to_tray),
                T_AUTO => self.core.toggle_autostart(),
                T_RESTORE => self.push(|c| c.restore_opacity = !c.restore_opacity),
                T_HOTKEY => self.push(|c| c.hotkeys = !c.hotkeys),
                T_PASS => self.push(|c| c.passthrough = !c.passthrough),
                T_TOP => self.push(|c| c.always_on_top = !c.always_on_top),
                _ => {}
            },
            Id::StepUp(i) => {
                let v = [1.0f32, 2.0, 5.0, 10.0][i];
                self.cfg.step_up = v;
                self.num_up = fmt_step(v);
                self.core.update(|c| c.step_up = v);
                self.repaint();
            }
            Id::StepDown(i) => {
                let v = [1.0f32, 2.0, 5.0, 10.0][i];
                self.cfg.step_down = v;
                self.num_dn = fmt_step(v);
                self.core.update(|c| c.step_down = v);
                self.repaint();
            }
            Id::HkChip(k) => {
                self.capture = Some(k);
                self.repaint();
            }
            Id::Osd(i) => {
                let mode = [OsdMode::Force, OsdMode::Compat, OsdMode::Off][i];
                self.push(|c| c.osd = mode);
            }
            Id::MonHead(i) => {
                if i < self.mon_open.len() {
                    self.mon_open[i] = !self.mon_open[i];
                }
                self.mon_menu = None;
                self.repaint();
            }
            Id::MonAct(i, k) => {
                self.mon_menu = if self.mon_menu == Some((i, k)) {
                    None
                } else {
                    Some((i, k))
                };
                self.repaint();
            }
            Id::MonActItem(i, k, m) => {
                if let Some(mm) = crate::winapi_util::monitors().get(i) {
                    let dev = mm.device.clone();
                    let mut list = self.cfg.mon_entries(&dev);
                    if let Some(e) = list.get_mut(k) {
                        e.act = m;
                    }
                    self.cfg.set_mon_entries(&dev, list.clone());
                    let dev2 = dev.clone();
                    self.core.update(move |c| c.set_mon_entries(&dev2, list));
                }
                self.mon_menu = None;
                self.repaint();
            }
            Id::MonDel(i, k) => {
                if let Some(mm) = crate::winapi_util::monitors().get(i) {
                    let dev = mm.device.clone();
                    let mut list = self.cfg.mon_entries(&dev);
                    if k < list.len() {
                        list.remove(k);
                    }
                    self.cfg.set_mon_entries(&dev, list.clone());
                    let dev2 = dev.clone();
                    self.core.update(move |c| c.set_mon_entries(&dev2, list));
                }
                self.mon_menu = None;
                self.mon_capture = None;
                self.repaint();
            }
            Id::MonAdd(i) => {
                if let Some(mm) = crate::winapi_util::monitors().get(i) {
                    let dev = mm.device.clone();
                    let mut list = self.cfg.mon_entries(&dev);
                    list.push(MonKey {
                        act: 0,
                        spec: String::new(),
                        step: self.cfg.step_up,
                    });
                    let idx = list.len() - 1;
                    self.cfg.set_mon_entries(&dev, list.clone());
                    let dev2 = dev.clone();
                    self.core.update(move |c| c.set_mon_entries(&dev2, list));
                    self.mon_capture = Some((i, idx));
                }
                self.mon_menu = None;
                self.repaint();
            }
            Id::MonKey(i, k) => {
                self.mon_capture = Some((i, k));
                self.mon_menu = None;
                self.repaint();
            }
            Id::MonStep(i, k, si) => {
                self.mon_step_edit = None;
                self.mon_step_text.clear();
                if let Some(mm) = crate::winapi_util::monitors().get(i) {
                    let dev = mm.device.clone();
                    let mut list = self.cfg.mon_entries(&dev);
                    if let Some(e) = list.get_mut(k) {
                        e.step = [1.0f32, 2.0, 5.0, 10.0][si.min(3)];
                    }
                    self.cfg.set_mon_entries(&dev, list.clone());
                    let dev2 = dev.clone();
                    self.core.update(move |c| c.set_mon_entries(&dev2, list));
                }
                self.repaint();
            }
            Id::Btn(0) => {
                let mut fresh = Config::default();
                fresh.displays = self.cfg.displays.clone();
                self.push(|c| *c = fresh.clone());
            }
            Id::Btn(1) => self.core.quit(),
            Id::Lang => {
                self.lang_open = !self.lang_open;
        if self.lang_open {
            self.lang_scroll = 0.0;
        }
            }
            Id::LangItem(i) => {
                let lang = crate::i18n::ALL[i];
                self.lang_open = false;
                self.cfg.lang = lang;
                self.draw.lang = lang;
                self.push(|c| c.lang = lang);
                if let Some(t) = self.tray.take() {
                    drop(t);
                }
                self.tray = crate::tray::init(&self.core, self.dark, lang);
                let title = wide(&format!("{} v{}", crate::i18n::title(lang), env!("CARGO_PKG_VERSION")));
                unsafe {
                    let _ = SetWindowTextW(self.hwnd, PCWSTR(title.as_ptr()));
                }
            }
            Id::Display(i) => {
                let Some(m) = self.monitors.get(i).cloned() else {
                    return;
                };
                let all: Vec<String> = self.monitors.iter().map(|m| m.device.clone()).collect();
                let mut sel = if !self.cfg.explicit_displays {
                    all.clone()
                } else {
                    self.cfg.displays.clone()
                };
                if sel.iter().any(|d| d == &m.device) {
                    sel.retain(|d| d != &m.device);
                } else {
                    sel.push(m.device.clone());
                }
                let (list, explicit) = if sel.len() == all.len() {
                    (Vec::new(), false)
                } else {
                    (sel, true)
                };
                self.push(|c| {
                    c.displays = list.clone();
                    c.explicit_displays = explicit;
                });
            }
            _ => {}
        }
        self.repaint();
    }
}

unsafe fn center_on(hmon: HMONITOR, w: i32, h: i32) -> (i32, i32) {
    unsafe {
        let mut work = RECT::default();
        let mut ok = false;
        if !hmon.is_invalid() {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(hmon, &mut info).as_bool() {
                work = info.rcWork;
                ok = true;
            }
        }
        if !ok {
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut work as *mut RECT as *mut c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
        let x = work.left + ((work.right - work.left) - w) / 2;
        let y = work.top + ((work.bottom - work.top) - h) / 2;
        (x.max(work.left), y.max(work.top))
    }
}

unsafe fn center_pos(w: i32, h: i32) -> (i32, i32) {
    unsafe {
        let mut pt = POINT::default();
        let mon = if GetCursorPos(&mut pt).is_ok() {
            MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST)
        } else {
            HMONITOR::default()
        };
        center_on(mon, w, h)
    }
}

pub fn run(core: Arc<Core>, for_dark: bool, tray: Option<TrayIcon>) {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).map(|m| m.0).unwrap_or_default());
        let class = wide(CLASS);
        let brush = CreateSolidBrush(cref(pal(for_dark).bg));
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: icon::hicon(for_dark, 32),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: brush,
            lpszMenuName: PCWSTR(std::ptr::null()),
            lpszClassName: PCWSTR(class.as_ptr()),
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }

        let s = winapi_util::system_scale();
        let mut gui = Gui::new(core.clone(), for_dark, tray, s);

        let style = WS_OVERLAPPEDWINDOW & !WS_MAXIMIZEBOX & !WS_MINIMIZEBOX | WS_SYSMENU;
        let client_h = DEFAULT_HEIGHT.max(400.0);
        gui.auto_h = false;
        gui.set_h = client_h as i32;
        let mut outer = RECT {
            left: 0,
            top: 0,
            right: gui.r(0.0, 0.0, W, 760.0).right,
            bottom: client_h as i32,
        };
        let _ = AdjustWindowRect(&mut outer, style, false);
        let width = outer.right - outer.left;
        let height = outer.bottom - outer.top;

        let title = wide(&format!("{} v{}", crate::i18n::title(core.snapshot().0.lang), env!("CARGO_PKG_VERSION")));
        let (pos_x, pos_y) = center_pos(width, height);
        let Ok(hwnd) = CreateWindowExW(
            WS_EX_APPWINDOW,
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            style,
            pos_x,
            pos_y,
            width,
            height,
            None,
            None,
            Some(hinstance),
            None,
        ) else {
            return;
        };
        gui.hwnd = hwnd;
        core.attach_hwnd(hwnd.0 as isize);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(Box::new(gui)) as isize);

        set_window_icons(hwnd, for_dark);
        let dark_flag: i32 = if for_dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(20),
            &dark_flag as *const i32 as *const c_void,
            4,
        );

        SetTimer(Some(hwnd), 1, 100, None);
        core.hide_window();

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe fn gui_of(hwnd: HWND) -> Option<&'static mut Gui> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Gui;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

fn refresh_monitors(g: &mut Gui) {
    let ms = winapi_util::monitors();
    let same = ms.len() == g.monitors.len()
        && ms.iter().zip(g.monitors.iter()).all(|(a, b)| {
            a.device == b.device
                && a.primary == b.primary
                && a.rect.left == b.rect.left
                && a.rect.top == b.rect.top
                && a.rect.right == b.rect.right
                && a.rect.bottom == b.rect.bottom
        });
    if !same {
        g.monitors = ms;
        g.repaint();
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => LRESULT(0),
            WM_PAINT => {
                let Some(g) = gui_of(hwnd) else {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                };
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut client = RECT::default();
                let _ = GetClientRect(hwnd, &mut client);
                let (w, h) = (client.right.max(1), client.bottom.max(1));

                let need_new = match &g.mem {
                    Some((_, _, mw, mh)) => *mw != w || *mh != h,
                    None => true,
                };
                if need_new {
                    if let Some((dc, bmp, _, _)) = g.mem.take() {
                        let _ = DeleteObject(HGDIOBJ(bmp as *mut c_void));
                        let _ = windows::Win32::Graphics::Gdi::DeleteDC(dc);
                    }
                    let mem = CreateCompatibleDC(Some(hdc));
                    let bmp = CreateCompatibleBitmap(hdc, w, h);
                    SelectObject(mem, HGDIOBJ(bmp.0));
                    g.mem = Some((mem, bmp.0 as isize, w, h));
                }
                if let Some((mem, _, _, _)) = g.mem {
                    paint_content(g, mem);
                    let _ = BitBlt(hdc, 0, 0, w, h, Some(mem), 0, 0, SRCCOPY);
                }
                let _ = EndPaint(hwnd, &ps);

                if g.content_h > 0 {
                    let style = WS_OVERLAPPEDWINDOW & !WS_MAXIMIZEBOX & !WS_MINIMIZEBOX | WS_SYSMENU;
                    let mut outer = RECT { left: 0, top: 0, right: g.r(0.0, 0.0, g.w(), 0.0).right, bottom: g.content_h };
                    let _ = AdjustWindowRect(&mut outer, style, false);
                    let frame_h = (outer.bottom - outer.top - g.content_h).max(0);
                    let mut work = RECT::default();
                    let _ = SystemParametersInfoW(
                        SPI_GETWORKAREA,
                        0,
                        Some(&mut work as *mut RECT as *mut c_void),
                        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
                    );
                    let work_h = (work.bottom - work.top).max(240);
                    let max_client = (work_h - frame_h - 40 * g.s() as i32).max((320.0 * g.s()) as i32);
                    let desired = if g.auto_h { g.content_h.min(max_client) } else { g.content_h.min(h) };
                    g.max_scroll = (g.content_h - desired).max(0);
                    if g.scroll > g.max_scroll {
                        g.scroll = g.max_scroll;
                    }
                    if g.scroll < 0 {
                        g.scroll = 0;
                    }
                    if g.auto_h && h != desired && h > 0 {
                        g.auto_h = false;
                        g.set_h = desired;
                        let mut wr = RECT::default();
                        let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut wr);
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            0,
                            0,
                            wr.right - wr.left,
                            outer.bottom - outer.top - (g.content_h - desired),
                            SWP_NOMOVE | SWP_NOZORDER,
                        );
                        let mut wr2 = RECT::default();
                        let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut wr2);
                        let (nx, ny) = center_on(
                            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                            wr2.right - wr2.left,
                            wr2.bottom - wr2.top,
                        );
                        let _ = SetWindowPos(hwnd, None, nx, ny, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
                    }
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_TIMER => {
                if let Some(g) = gui_of(hwnd) {
                    g.tick = g.tick.wrapping_add(1);
                    if g.tick % 10 == 0 {
                        refresh_monitors(g);
                    }
                    let (_, rev) = g.core.snapshot();
                    if rev != g.rev {
                        let (cfg, rev) = g.core.snapshot();
                        g.cfg = cfg;
                        g.rev = rev;
                        if !g.editing {
                            g.num = format!("{}", g.cfg.opacity.round() as i32);
                            g.num_up = fmt_step(g.cfg.step_up);
                            g.num_dn = fmt_step(g.cfg.step_down);
                        }
                        g.apply_theme();
                        g.repaint();
                    } else {
                        g.core.flush_due();
                        if g.editing {
                            let raw = unsafe { GetCaretBlinkTime() };
                            let ms = if raw == 0 || raw == u32::MAX { 530 } else { raw } as u128;
                            if g.blink_at.elapsed().as_millis() >= ms {
                                g.caret = !g.caret;
                                g.blink_at = Instant::now();
                                g.repaint();
                            }
                        }
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if let Some(g) = gui_of(hwnd) {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    if g.pressed == Some(Id::Slider) {
                        let held = unsafe {
                            windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x01) as u16 & 0x8000 != 0
                        };
                        if held {
                            let v = g.slider_value(x);
                            g.set_level(v);
                        } else {
                            let _ = unsafe { ReleaseCapture() };
                            g.pressed = None;
                            g.repaint();
                        }
                    } else {
                        let id = g.hit(x, y);
                        if id != g.hover {
                            g.hover = id;
                            g.repaint();
                        }
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                if let Some(g) = gui_of(hwnd) {
                    let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    if g.lang_open && g.lang_max_scroll > 0.0 {
                        g.lang_scroll = (g.lang_scroll - delta as f32 / 120.0 * 24.0 * g.s())
                            .clamp(0.0, g.lang_max_scroll);
                        g.repaint();
                    } else if g.max_scroll > 0 {
                        let step = (52.0 * g.s()) as i32 * 3;
                        g.scroll = (g.scroll - delta / 120 * step).clamp(0, g.max_scroll);
                        g.repaint();
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                if let Some(g) = gui_of(hwnd) {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let id = g.hit(x, y);
                    let same_global = match id {
                        Some(Id::Input) => g.editing && g.edit_kind == 0,
                        Some(Id::StepIn(k)) => g.editing && g.edit_kind == k,
                        _ => false,
                    };
                    if g.editing && !same_global {
                        g.commit_input();
                    }
                    let same_mon = match id {
                        Some(Id::MonStepIn(i, k)) => g.mon_step_edit == Some((i, k)),
                        _ => false,
                    };
                    if g.mon_step_edit.is_some() && !same_mon {
                        g.commit_mon_step();
                    }
                    if g.lang_open && !matches!(id, Some(Id::Lang) | Some(Id::LangItem(_))) {
                        g.lang_open = false;
                    }
                    if g.mon_menu.is_some()
                        && !matches!(id, Some(Id::MonAct(_, _)) | Some(Id::MonActItem(_, _, _)))
                    {
                        g.mon_menu = None;
                        g.repaint();
                    }
                    g.pressed = id;
                    match id {
                        Some(Id::Slider) => {
                            g.focus = Some(Id::Slider);
                            unsafe {
                                let _ = SetCapture(hwnd);
                            }
                            let v = g.slider_value(x);
                            g.set_level(v);
                        }
                        Some(Id::Input) => {
                            g.focus = Some(Id::Input);
                            g.editing = true;
                            g.caret = true;
                            g.blink_at = Instant::now();
                            g.edit_kind = 0;
                            g.select_all = true;
                            g.num = format!("{}", g.level().round() as i32);
                        }
                        Some(Id::MonStepIn(i, k)) => {
                            g.editing = false;
                            g.mon_step_edit = Some((i, k));
                            let mut text = String::new();
                            if let Some(mm) = crate::winapi_util::monitors().get(i) {
                                if let Some(e) = g.cfg.mon_entries(&mm.device).get(k) {
                                    text = fmt_step(e.step);
                                }
                            }
                            g.mon_step_text = text;
                            g.select_all = true;
                            g.caret = true;
                            g.blink_at = Instant::now();
                        }
                        Some(Id::StepIn(k)) => {
                            g.focus = Some(Id::StepIn(k));
                            g.editing = true;
                            g.caret = true;
                            g.blink_at = Instant::now();
                            g.edit_kind = k;
                            g.select_all = true;
                            if k == 1 {
                                g.num_up = fmt_step(g.cfg.step_up);
                            } else {
                                g.num_dn = fmt_step(g.cfg.step_down);
                            }
                        }
                        Some(other) => {
                            g.focus = None;
                            g.on_click(other);
                        }
                        None => {}
                    }
                    g.repaint();
                }
                LRESULT(0)
            }
            WM_LBUTTONUP | 0x0215 => {
                let _ = unsafe { ReleaseCapture() };
                if let Some(g) = gui_of(hwnd) {
                    g.pressed = None;
                    g.repaint();
                }
                LRESULT(0)
            }
            WM_CHAR => {
                if let Some(g) = gui_of(hwnd) {
                    if g.mon_step_edit.is_some() {
                        let ch = wparam.0 as u32;
                        let mut text = g.mon_step_text.clone();
                        if ch == 0x0D {
                            g.commit_mon_step();
                        } else if ch == 0x1B {
                            g.mon_step_edit = None;
                            g.mon_step_text.clear();
                        } else if ch == 0x08 {
                            if g.select_all {
                                text.clear();
                                g.select_all = false;
                            } else {
                                text.pop();
                            }
                            g.mon_step_text = text;
                        } else if (0x30..=0x39).contains(&ch) || (ch == 0x2E && !text.contains('.')) {
                            if g.select_all {
                                text.clear();
                                g.select_all = false;
                            }
                            if text.len() < 4 {
                                if text == "0" && ch != 0x2E {
                                    text.clear();
                                }
                                text.push(ch as u8 as char);
                            }
                            g.mon_step_text = text;
                        }
                        g.repaint();
                        return LRESULT(0);
                    }
                    if g.editing {
                        let ch = wparam.0 as u32;
                        let mut text = g.buf_get();
                        if ch == 0x0D {
                            g.commit_input();
                        } else if ch == 0x1B {
                            g.editing = false;
                            g.select_all = false;
                            g.edit_kind = 0;
                            g.num = format!("{}", g.level().round() as i32);
                            g.num_up = fmt_step(g.cfg.step_up);
                            g.num_dn = fmt_step(g.cfg.step_down);
                        } else if ch == 0x09 {
                            g.commit_input();
                            g.focus = Some(Id::Slider);
                        } else if ch == 0x08 {
                            if g.select_all {
                                text.clear();
                                g.select_all = false;
                            } else {
                                text.pop();
                            }
                            g.buf_set(text);
                        } else if (0x30..=0x39).contains(&ch)
                            || (ch == 0x2E && g.edit_kind != 0 && !text.contains('.'))
                        {
                            if g.select_all {
                                text.clear();
                                g.select_all = false;
                            }
                            let max = if g.edit_kind == 0 { 3 } else { 4 };
                            if text.len() < max {
                                if text == "0" && ch != 0x2E {
                                    text.clear();
                                }
                                text.push(ch as u8 as char);
                            }
                            g.buf_set(text);
                        }
                        g.repaint();
                    }
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if let Some(g) = gui_of(hwnd) {
                    let vk = wparam.0 as u16;
                    if let Some((mi, ei)) = g.mon_capture {
                        use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
                        let down = |v: u16| unsafe { GetKeyState(v as i32) as u16 & 0x8000 != 0 };
                        if vk == VK_ESCAPE.0 {
                            g.mon_capture = None;
                            g.repaint();
                            return LRESULT(0);
                        }
                        if [0x10u16, 0x11, 0x12, 0x5B, 0x5C].contains(&vk) {
                            return LRESULT(0);
                        }
                        let mut mods = 0u32;
                        if down(0x11) {
                            mods |= crate::hotkey::M_CTRL;
                        }
                        if down(0x12) {
                            mods |= crate::hotkey::M_ALT;
                        }
                        if down(0x10) {
                            mods |= crate::hotkey::M_SHIFT;
                        }
                        if down(0x5B) || down(0x5C) {
                            mods |= crate::hotkey::M_WIN;
                        }
                        if mods != 0 {
                            let spec = crate::hotkey::format(mods, vk as u32);
                            if let Some(mm) = crate::winapi_util::monitors().get(mi) {
                                let dev = mm.device.clone();
                                let mut list = g.cfg.mon_entries(&dev);
                                if let Some(e) = list.get_mut(ei) {
                                    e.spec = spec;
                                }
                                g.cfg.set_mon_entries(&dev, list.clone());
                                let dev2 = dev.clone();
                                g.core.update(move |c| c.set_mon_entries(&dev2, list));
                            }
                            g.mon_capture = None;
                            g.repaint();
                        }
                        return LRESULT(0);
                    }
                    if let Some(k) = g.capture {
                        use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
                        let down = |v: u16| unsafe { GetKeyState(v as i32) as u16 & 0x8000 != 0 };
                        if vk == VK_ESCAPE.0 {
                            g.capture = None;
                            g.repaint();
                            return LRESULT(0);
                        }
                        if [0x10u16, 0x11, 0x12, 0x5B, 0x5C].contains(&vk) {
                            return LRESULT(0);
                        }
                        let mut mods = 0u32;
                        if down(0x11) {
                            mods |= crate::hotkey::M_CTRL;
                        }
                        if down(0x12) {
                            mods |= crate::hotkey::M_ALT;
                        }
                        if down(0x10) {
                            mods |= crate::hotkey::M_SHIFT;
                        }
                        if down(0x5B) || down(0x5C) {
                            mods |= crate::hotkey::M_WIN;
                        }
                        if mods != 0 {
                            let spec = crate::hotkey::format(mods, vk as u32);
                            g.capture = None;
                            match k {
                                0 => g.cfg.hk_inc = spec.clone(),
                                1 => g.cfg.hk_dec = spec.clone(),
                                2 => g.cfg.hk_off = spec.clone(),
                                _ => g.cfg.hk_toggle = spec.clone(),
                            }
                            g.core.update(|c| match k {
                                0 => c.hk_inc = spec.clone(),
                                1 => c.hk_dec = spec.clone(),
                                2 => c.hk_off = spec.clone(),
                                _ => c.hk_toggle = spec.clone(),
                            });
                            g.repaint();
                        }
                        return LRESULT(0);
                    }
                    if g.editing {
                        if vk == VK_BACK.0 {
                            let mut text = g.buf_get();
                            if g.select_all {
                                text.clear();
                                g.select_all = false;
                            } else {
                                text.pop();
                            }
                            g.buf_set(text);
                            g.repaint();
                        } else if vk == VK_TAB.0 {
                            g.commit_input();
                            g.focus = Some(Id::Slider);
                        }
                    } else if g.focus == Some(Id::Slider) {
                        let shift = unsafe {
                            windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(
                                windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT.0 as i32,
                            ) as u16
                                & 0x8000
                                != 0
                        };
                        let step = if shift { 10.0 } else { 1.0 };
                        let cur = g.level();
                        let next = match vk {
                            v if v == VK_LEFT.0 => Some(cur - step),
                            v if v == VK_RIGHT.0 => Some(cur + step),
                            v if v == VK_UP.0 => Some(cur + step),
                            v if v == VK_DOWN.0 => Some(cur - step),
                            v if v == VK_HOME.0 => Some(0.0),
                            v if v == VK_END.0 => Some(MAX_LEVEL),
                            _ => None,
                        };
                        if let Some(v) = next {
                            g.set_level(v);
                        }
                    }
                    if vk == VK_TAB.0 && !g.editing {
                        g.focus = Some(Id::Input);
                        g.editing = true;
                        g.caret = true;
                        g.blink_at = Instant::now();
                        g.select_all = true;
                        g.num = format!("{}", g.level().round() as i32);
                        g.edit_kind = 0;
                        g.repaint();
                    }
                }
                LRESULT(0)
            }
            0x0317 | 0x0318 => LRESULT(0),
            WM_SYSKEYDOWN => DefWindowProcW(hwnd, msg, wparam, lparam),
            WM_SIZE => {
                if let Some(g) = gui_of(hwnd) {
                    let mut rc = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rc);
                    if g.set_h > 0 && rc.bottom != g.set_h {
                        g.auto_h = false;
                    }
                    g.repaint();
                }
                LRESULT(0)
            }
            WM_ENTERSIZEMOVE | WM_EXITSIZEMOVE => {
                if let Some(g) = gui_of(hwnd) {
                    g.auto_h = false;
                }
                LRESULT(0)
            }
            WM_GETMINMAXINFO => {
                let info = lparam.0 as *mut MINMAXINFO;
                if !info.is_null() {
                    let dpi = unsafe { GetDpiForWindow(hwnd) };
                    let s = if dpi == 0 {
                        crate::winapi_util::system_scale()
                    } else {
                        dpi as f32 / 96.0
                    };
                    (*info).ptMinTrackSize.x = (508.0 * s) as i32;
                    (*info).ptMinTrackSize.y = (520.0 * s) as i32;
                }
                LRESULT(0)
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE => {
                if let Some(g) = gui_of(hwnd) {
                    refresh_monitors(g);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let Some(g) = gui_of(hwnd) else {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                };
                if g.cfg.close_to_tray {
                    g.core.hide_window();
                    LRESULT(0)
                } else {
                    g.core.quit();
                    LRESULT(0)
                }
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => {
                let res = DefWindowProcW(hwnd, msg, wparam, lparam);
                if msg == 0x0003 {
                }
                res
            }
        }
    }
}


