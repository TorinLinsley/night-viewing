// 纯黑遮罩窗口：每个显示器一个分层窗口 + 全局热键
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, CreateSolidBrush, EndPaint, FillRect, HBRUSH, UpdateWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey,
    UnregisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, GetWindowLongPtrW, IsWindowVisible, LoadCursorW, PostMessageW, PostQuitMessage,
    RegisterClassW, SetLayeredWindowAttributes, SetTimer, SetWindowDisplayAffinity, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, TranslateMessage, CS_DBLCLKS, MSG, SW_HIDE,
    SW_SHOWNOACTIVATE, WINDOW_EX_STYLE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, GWL_EXSTYLE, HWND_TOPMOST,
    IDC_ARROW, LWA_ALPHA, WDA_EXCLUDEFROMCAPTURE, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_HOTKEY,
    WM_PAINT, WM_TIMER,
};

use crate::config::{Config, MAX_LEVEL};
use crate::core::Core;
use crate::winapi_util;

const OVERLAY_CLASS: &str = "NightViewingOverlay::Window";
const CONTROL_CLASS: &str = "NightViewingOverlay::Control";
const WINDOW_TITLE: &str = "Night Viewing Overlay";

const HK_INC: i32 = 1;
const HK_DEC: i32 = 2;
const HK_RESET: i32 = 3;
const HK_TOGGLE: i32 = 4;

static ALPHA: AtomicU8 = AtomicU8::new(0);
static WANT_VISIBLE: AtomicBool = AtomicBool::new(false);
static HIDDEN: AtomicBool = AtomicBool::new(false);
static TOPMOST: AtomicBool = AtomicBool::new(true);
static PASSTHROUGH: AtomicBool = AtomicBool::new(true);
static HOTKEYS: AtomicBool = AtomicBool::new(true);
static HOTKEYS_ON: AtomicBool = AtomicBool::new(false);
static CONTROL: AtomicIsize = AtomicIsize::new(0);
static WINDOWS: Mutex<Vec<(isize, String)>> = Mutex::new(Vec::new());
static SELECTION: Mutex<(bool, Vec<String>)> = Mutex::new((false, Vec::new()));
static LEVELS: Mutex<Vec<(String, f32)>> = Mutex::new(Vec::new());
static BRUSH: AtomicUsize = AtomicUsize::new(0);
static CORE: OnceLock<Arc<Core>> = OnceLock::new();
static HK_KEYS: [AtomicU64; 4] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
static HK_SIG: AtomicU64 = AtomicU64::new(u64::MAX);
static MON_REG: Mutex<Vec<(i32, String, u8, f32, u32, u32)>> = Mutex::new(Vec::new());
static REG_IDS: Mutex<Vec<i32>> = Mutex::new(Vec::new());
static LAST_STRUCT: Mutex<String> = Mutex::new(String::new());

pub fn apply(cfg: &Config) {
    let pct = cfg.opacity.clamp(0.0, MAX_LEVEL);
    let alpha = (pct * 255.0 / 100.0).round() as u8;
    ALPHA.store(alpha, Ordering::Relaxed);
    HIDDEN.store(cfg.hide, Ordering::Relaxed);
    WANT_VISIBLE.store(!cfg.hide && alpha > 0, Ordering::Relaxed);
    TOPMOST.store(cfg.always_on_top, Ordering::Relaxed);
    PASSTHROUGH.store(cfg.passthrough, Ordering::Relaxed);
    HOTKEYS.store(cfg.hotkeys, Ordering::Relaxed);
    let specs = [&cfg.hk_inc, &cfg.hk_dec, &cfg.hk_off, &cfg.hk_toggle];
    for (i, spec) in specs.iter().enumerate() {
        let packed = crate::hotkey::parse(spec)
            .map(|(mods, vk)| ((mods as u64) << 32) | vk as u64)
            .unwrap_or(0);
        HK_KEYS[i].store(packed, Ordering::Relaxed);
    }
    let mut mon_sig = String::new();
    for (dev, list) in cfg.mon_hk.iter() {
        mon_sig.push_str(dev);
        mon_sig.push('|');
        for e in list {
            mon_sig.push_str(&format!("{},{},{},{};", e.act, e.step, e.spec, crate::hotkey::parse(&e.spec).map(|(m, v)| (m, v)).unwrap_or((0, 0)).0));
        }
    }
    let structural = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        cfg.hotkeys,
        cfg.passthrough,
        cfg.always_on_top,
        cfg.hk_inc,
        cfg.hk_dec,
        cfg.hk_off,
        cfg.hk_toggle,
        mon_sig,
        cfg.explicit_displays,
        cfg.displays.join(";"),
        winapi_util::monitor_generation()
    );
    let changed = LAST_STRUCT
        .lock()
        .map(|s| s.as_str() != structural)
        .unwrap_or(true);
    if changed {
        if let Ok(mut reg) = MON_REG.lock() {
            reg.clear();
            for (i, m) in winapi_util::monitors().iter().enumerate().take(8) {
                if let Some(list) = cfg.mon_hk.get(&m.device) {
                    for (k, e) in list.iter().enumerate().take(8) {
                        if let Some((mods, vk)) = crate::hotkey::parse(&e.spec) {
                            reg.push((100 + (i as i32) * 10 + k as i32, m.device.clone(), e.act, e.step, mods, vk));
                        }
                    }
                }
            }
        }
        if let Ok(mut s) = LAST_STRUCT.lock() {
            *s = structural;
        }
    }
    if let Ok(mut guard) = SELECTION.lock() {
        *guard = (cfg.explicit_displays, cfg.displays.clone());
    }
    if let Ok(mut guard) = LEVELS.lock() {
        *guard = cfg
            .levels
            .iter()
            .map(|(d, v)| (d.clone(), v.clamp(0.0, MAX_LEVEL)))
            .collect();
    }
}

pub fn spawn(core: Arc<Core>) {
    let _ = CORE.set(core);
    std::thread::Builder::new()
        .name("night-viewing-overlay".into())
        .stack_size(256 * 1024)
        .spawn(run)
        .ok();
}

pub fn shutdown() {
    let hwnd = CONTROL.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn log(tag: &str, detail: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("night_viewing_overlay.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{tag}: {detail}");
    }
}

fn selected_monitors() -> Vec<winapi_util::Monitor> {
    let (explicit, sel) = SELECTION
        .lock()
        .map(|s| s.clone())
        .unwrap_or((false, Vec::new()));
    winapi_util::monitors()
        .into_iter()
        .filter(|m| !explicit || sel.iter().any(|d| d == &m.device))
        .collect()
}

fn ex_style() -> WINDOW_EX_STYLE {
    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
}

unsafe fn create_overlay(clazz: &[u16], title: &[u16], hinstance: HINSTANCE, r: &RECT) -> isize {
    unsafe {
        let Ok(hwnd) = CreateWindowExW(
            ex_style(),
            PCWSTR(clazz.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            r.left,
            r.top,
            (r.right - r.left).max(1),
            (r.bottom - r.top).max(1),
            None,
            None,
            Some(hinstance),
            None,
        ) else {
            return 0;
        };
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
        if SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE).is_err() {
            log("capture-exclusion", "unsupported");
        }
        hwnd.0 as isize
    }
}

unsafe fn rebuild(clazz: &[u16], title: &[u16], hinstance: HINSTANCE) {
    unsafe {
        let wanted = selected_monitors();
        let current: Vec<String> = WINDOWS
            .lock()
            .map(|w| w.iter().map(|x| x.1.clone()).collect())
            .unwrap_or_default();
        let same = current.len() == wanted.len()
            && wanted.iter().all(|m| current.iter().any(|c| c == &m.device));
        if same && !current.is_empty() {
            apply_visual();
            return;
        }
        if let Ok(mut guard) = WINDOWS.lock() {
            for (h, _) in guard.iter() {
                let _ = DestroyWindow(HWND(*h as *mut _));
            }
            guard.clear();
            for m in &wanted {
                let h = create_overlay(clazz, title, hinstance, &m.rect);
                if h != 0 {
                    guard.push((h, m.device.clone()));
                }
            }
        }
        let summary: Vec<String> = wanted
            .iter()
            .map(|m| format!("{} {}x{}", m.device, m.width(), m.height()))
            .collect();
        log("overlays", &summary.join(" | "));
        apply_visual();
    }
}

unsafe fn apply_visual() {
    unsafe {
        let alpha = ALPHA.load(Ordering::Relaxed);
        let want = WANT_VISIBLE.load(Ordering::Relaxed);
        let topmost = TOPMOST.load(Ordering::Relaxed);
        let passthrough = PASSTHROUGH.load(Ordering::Relaxed);

        let levels = LEVELS.lock().map(|l| l.clone()).unwrap_or_default();
        let guard = match WINDOWS.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        for (h, device) in guard.iter() {
            let hwnd = HWND(*h as *mut _);
            let own = levels
                .iter()
                .find(|(d, _)| d == device)
                .map(|(_, v)| *v)
                .unwrap_or(MAX_LEVEL + 1.0);
            let (alpha, want) = if own <= MAX_LEVEL {
                let a = (own.clamp(0.0, MAX_LEVEL) * 255.0 / 100.0).round() as u8;
                (a, a > 0)
            } else {
                (alpha, want)
            };
            let (alpha, want) = if HIDDEN.load(Ordering::Relaxed) {
                (0u8, false)
            } else {
                (alpha, want)
            };
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
            if IsWindowVisible(hwnd).as_bool() != want {
                let _ = ShowWindow(hwnd, if want { SW_SHOWNOACTIVATE } else { SW_HIDE });
            }
            if want {
                let _ = UpdateWindow(hwnd);
            }
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            let has = ex & WS_EX_TRANSPARENT.0 != 0;
            if passthrough != has {
                let mut next = ex;
                if passthrough {
                    next |= WS_EX_TRANSPARENT.0;
                } else {
                    next &= !WS_EX_TRANSPARENT.0;
                }
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next as isize);
            }
            if topmost {
                let _ = SetWindowPos(
                    hwnd,
                    crate::osd::anchor().or(Some(HWND_TOPMOST)),
                    0,
                    0,
                    0,
                    0,
                    windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                        | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                        | windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
                );
            }
        }
    }
}

unsafe fn sync_hotkeys(control: HWND) {
    unsafe {
        let want = HOTKEYS.load(Ordering::Relaxed);
        let mut plan: Vec<(i32, u32, u32)> = Vec::new();
        if want {
            for (i, id) in [HK_INC, HK_DEC, HK_RESET, HK_TOGGLE].iter().enumerate() {
                let packed = HK_KEYS[i].load(Ordering::Relaxed);
                if packed == 0 {
                    continue;
                }
                plan.push((*id, (packed >> 32) as u32, (packed & 0xFFFF_FFFF) as u32));
            }
            if let Ok(reg) = MON_REG.lock() {
                for (id, _dev, _act, _step, mods, vk) in reg.iter() {
                    plan.push((*id, *mods, *vk));
                }
            }
        }
        let sig = plan.iter().fold(want as u64, |acc, (id, mods, vk)| {
            acc.wrapping_mul(131)
                .wrapping_add(*id as u64)
                .wrapping_mul(131)
                .wrapping_add(((*mods as u64) << 32) | *vk as u64)
        });
        if sig == HK_SIG.load(Ordering::Relaxed) {
            return;
        }
        if let Ok(mut ids) = REG_IDS.lock() {
            for id in ids.drain(..) {
                let _ = UnregisterHotKey(Some(control), id);
            }
        }
        if want {
            for (id, mods, vk) in plan {
                match RegisterHotKey(Some(control), id, HOT_KEY_MODIFIERS(mods), vk) {
                    Ok(()) => {
                        log("hotkey", &format!("{id} ok mods={mods} vk={vk}"));
                        if let Ok(mut ids) = REG_IDS.lock() {
                            ids.push(id);
                        }
                    }
                    Err(e) => log("hotkey-fail", &format!("{id} mods={mods} vk={vk} err={e}")),
                }
            }
        }
        HOTKEYS_ON.store(want, Ordering::Relaxed);
        HK_SIG.store(sig, Ordering::Relaxed);
    }
}

fn run() {
    unsafe {
        let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let hinstance = HINSTANCE(GetModuleHandleW(None).map(|m| m.0).unwrap_or_default());
        let brush: HBRUSH = CreateSolidBrush(COLORREF(0));
        BRUSH.store(brush.0 as usize, Ordering::SeqCst);

        let overlay_class = wide(OVERLAY_CLASS);
        let control_class = wide(CONTROL_CLASS);
        let title = wide(WINDOW_TITLE);

        let base = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(overlay_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: Default::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: brush,
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(overlay_class.as_ptr()),
        };
        if RegisterClassW(&base) == 0 {
            return;
        }
        let control_wc = WNDCLASSW {
            lpfnWndProc: Some(control_proc),
            hbrBackground: Default::default(),
            lpszClassName: PCWSTR(control_class.as_ptr()),
            ..base
        };
        if RegisterClassW(&control_wc) == 0 {
            return;
        }

        let Ok(control) = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            PCWSTR(control_class.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(hinstance),
            None,
        ) else {
            return;
        };
        CONTROL.store(control.0 as isize, Ordering::SeqCst);

        rebuild(&overlay_class, &title, hinstance);
        sync_hotkeys(control);
        SetTimer(Some(control), 0x4E56, 60, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn handle_hotkey(raw: i32) {
    if raw >= 100 {
        if let Some(core) = CORE.get() {
            let entry = MON_REG.lock().ok().and_then(|reg| {
                reg.iter()
                    .find(|(id, ..)| *id == raw)
                    .map(|(_, d, a, s, _, _)| (d.clone(), *a, *s))
            });
            if let Some((device, act, step)) = entry {
                let cur = core.level_of(&device);
                match act {
                    0 => core.set_level(Some(&device), (cur + step).clamp(0.0, MAX_LEVEL)),
                    1 => core.set_level(Some(&device), (cur - step).clamp(0.0, MAX_LEVEL)),
                    _ => core.set_level(Some(&device), 0.0),
                }
                crate::osd::notify_hint();
                log("hotkey", &format!("mon act={act} level={}", core.level_of(&device)));
            }
        }
        return;
    }
    let Some(core) = CORE.get() else {
        return;
    };
    log("hotkey", &format!("msg id={raw}"));
    let id = if raw > 4 { ((raw - 11) % 10) + 1 } else { raw };
    {
        match id {
            HK_INC => {
                core.nudge_step(1.0);
                crate::osd::notify_hint();
            }
            HK_DEC => {
                core.nudge_step(-1.0);
                crate::osd::notify_hint();
            }
            HK_RESET => {
                core.toggle_hide();
            }
            HK_TOGGLE => core.toggle_window(),
            _ => {}
        }
        log(
            "hotkey",
            &format!("id={id} opacity={}", core.snapshot().0.opacity),
        );
    }
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_HOTKEY => {
                log("hotkey", &format!("overlay-hwnd got id={}", wparam.0));
                handle_hotkey(wparam.0 as i32);
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let brush = HBRUSH(BRUSH.load(Ordering::SeqCst) as *mut _);
                let rect = RECT {
                    left: 0,
                    top: 0,
                    right: ps.rcPaint.right,
                    bottom: ps.rcPaint.bottom,
                };
                FillRect(hdc, &rect, brush);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe extern "system" fn control_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_TIMER => {
                sync_hotkeys(hwnd);
                apply_visual();
                let (explicit, sel) = SELECTION
                    .lock()
                    .map(|s| s.clone())
                    .unwrap_or((false, Vec::new()));
                let count = WINDOWS.lock().map(|w| w.len()).unwrap_or(0);
                let expected = winapi_util::monitors()
                    .into_iter()
                    .filter(|m| !explicit || sel.iter().any(|d| d == &m.device))
                    .count();
                if count != expected {
                    let overlay_class = wide(OVERLAY_CLASS);
                    let title = wide(WINDOW_TITLE);
                    let hinstance = HINSTANCE(GetModuleHandleW(None).map(|m| m.0).unwrap_or_default());
                    rebuild(&overlay_class, &title, hinstance);
                }
                LRESULT(0)
            }
            WM_DISPLAYCHANGE => {
                winapi_util::invalidate_monitors();
                let overlay_class = wide(OVERLAY_CLASS);
                let title = wide(WINDOW_TITLE);
                let hinstance = HINSTANCE(GetModuleHandleW(None).map(|m| m.0).unwrap_or_default());
                if let Ok(mut guard) = WINDOWS.lock() {
                    for (h, _) in guard.iter() {
                        let _ = DestroyWindow(HWND(*h as *mut _));
                    }
                    guard.clear();
                }
                rebuild(&overlay_class, &title, hinstance);
                LRESULT(0)
            }
            WM_HOTKEY => {
                handle_hotkey(wparam.0 as i32);
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
