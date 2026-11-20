// 应用内核：配置唯一真相源 + 遮罩 / 窗口 / 托盘的桥接
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    IsIconic, SetForegroundWindow, ShowWindow, SW_HIDE, SW_RESTORE, SW_SHOWNORMAL,
};

use crate::config::{Config, MAX_LEVEL};
use crate::overlay;
use crate::winapi_util;

pub struct Core {
    cfg: Mutex<Config>,
    rev: AtomicU64,
    hwnd: AtomicIsize,
    dirty: AtomicBool,
    last_save: Mutex<Instant>,
    window_hidden: AtomicBool,
    quitting: AtomicBool,
}

impl Core {
    pub fn new() -> Arc<Self> {
        let cfg = Config::load();
        overlay::apply(&cfg);
        if cfg.autostart {
            winapi_util::set_autostart(true);
        }
        Arc::new(Self {
            cfg: Mutex::new(cfg),
            rev: AtomicU64::new(1),
            hwnd: AtomicIsize::new(0),
            dirty: AtomicBool::new(false),
            last_save: Mutex::new(Instant::now() - Duration::from_secs(5)),
            window_hidden: AtomicBool::new(false),
            quitting: AtomicBool::new(false),
        })
    }

    pub fn attach_hwnd(&self, hwnd: isize) {
        self.hwnd.store(hwnd, Ordering::SeqCst);
    }

    fn hwnd(&self) -> HWND {
        HWND(self.hwnd.load(Ordering::SeqCst) as *mut _)
    }

    pub fn show_window(&self) {
        self.window_hidden.store(false, Ordering::Relaxed);
        let hwnd = self.hwnd();
        if hwnd.0.is_null() {
            return;
        }
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }

    pub fn hide_window(&self) {
        self.window_hidden.store(true, Ordering::Relaxed);
        let hwnd = self.hwnd();
        if !hwnd.0.is_null() {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    pub fn toggle_window(&self) {
        if self.window_hidden.load(Ordering::Relaxed) {
            self.show_window();
        } else {
            self.hide_window();
        }
    }

    pub fn snapshot(&self) -> (Config, u64) {
        let cfg = self.cfg.lock().unwrap().clone();
        (cfg, self.rev.load(Ordering::Acquire))
    }

    pub fn update(&self, f: impl FnOnce(&mut Config)) {
        let updated = {
            let mut guard = self.cfg.lock().unwrap();
            f(&mut guard);
            guard.opacity = guard.opacity.clamp(0.0, MAX_LEVEL);
            guard.clone()
        };
        overlay::apply(&updated);
        self.rev.fetch_add(1, Ordering::AcqRel);
        self.dirty.store(true, Ordering::Relaxed);
        self.maybe_save(&updated);
    }

    pub fn set_opacity(&self, value: f32) {
        self.update(|c| c.opacity = value.clamp(0.0, MAX_LEVEL));
    }

    pub fn nudge_step(&self, dir: f32) {
        self.nudge_level(None, dir);
    }

    pub fn step_of(&self, dir: f32) -> f32 {
        let guard = self.cfg.lock().unwrap();
        if dir >= 0.0 { guard.step_up } else { guard.step_down }
    }

    pub fn level_of(&self, device: &str) -> f32 {
        self.cfg.lock().unwrap().level_for(device)
    }

    pub fn set_level(&self, device: Option<&str>, value: f32) {
        let value = value.clamp(0.0, MAX_LEVEL);
        match device {
            None => self.update(|c| {
                c.opacity = value;
                for level in c.levels.values_mut() {
                    *level = value;
                }
            }),
            Some(dev) => {
                let dev = dev.to_string();
                self.update(move |c| {
                    c.levels.insert(dev, value);
                })
            }
        }
    }

    pub fn nudge_level(&self, device: Option<&str>, dir: f32) {
        let step = self.step_of(dir);
        match device {
            None => self.update(|c| {
                c.opacity = (c.opacity + dir * step).clamp(0.0, MAX_LEVEL);
                for level in c.levels.values_mut() {
                    *level = (*level + dir * step).clamp(0.0, MAX_LEVEL);
                }
            }),
            Some(dev) => {
                let current = self.level_of(dev);
                self.set_level(Some(dev), current + dir * step);
            }
        }
    }

    pub fn nudge(&self, delta: f32) {
        self.update(|c| c.opacity = (c.opacity + delta).clamp(0.0, MAX_LEVEL));
    }

    pub fn toggle_autostart(&self) {
        let enabled = {
            let guard = self.cfg.lock().unwrap();
            !guard.autostart
        };
        let applied = winapi_util::set_autostart(enabled);
        self.update(|c| c.autostart = enabled && applied);
    }

    fn maybe_save(&self, cfg: &Config) {
        let mut last = self.last_save.lock().unwrap();
        if last.elapsed() >= Duration::from_millis(400) {
            *last = Instant::now();
            self.dirty.store(false, Ordering::Relaxed);
            cfg.save();
        }
    }

    pub fn flush_due(&self) {
        if !self.dirty.load(Ordering::Relaxed) {
            return;
        }
        let due = self.last_save.lock().unwrap().elapsed() >= Duration::from_millis(400);
        if due {
            let cfg = self.cfg.lock().unwrap().clone();
            self.maybe_save(&cfg);
        }
    }

    pub fn quitting(&self) -> bool {
        self.quitting.load(Ordering::Relaxed)
    }

    pub fn quit(&self) {
        self.quitting.store(true, Ordering::Relaxed);
        let cfg = self.cfg.lock().unwrap().clone();
        cfg.save();
        overlay::shutdown();
        std::thread::sleep(Duration::from_millis(40));
        std::process::exit(0);
    }
}
