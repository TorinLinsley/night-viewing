// 配置项与本地持久化
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::i18n::Lang;

pub const MAX_LEVEL: f32 = 90.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    System,
    Dark,
    Light,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Dark => "深色",
            Self::Light => "浅色",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OsdMode {
    Force,
    Compat,
    Off,
}

impl OsdMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Force => "强制显示",
            Self::Compat => "最大兼容",
            Self::Off => "禁用",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonKey {
    #[serde(default)]
    pub act: u8,
    #[serde(default)]
    pub spec: String,
    #[serde(default = "default_step")]
    pub step: f32,
}

fn default_step() -> f32 {
    5.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub opacity: f32,
    pub hide: bool,
    pub theme: ThemeMode,
    pub close_to_tray: bool,
    pub passthrough: bool,
    pub always_on_top: bool,
    pub autostart: bool,
    pub restore_opacity: bool,
    pub hotkeys: bool,
    pub displays: Vec<String>,
    pub explicit_displays: bool,
    pub lang: Lang,
    pub hk_inc: String,
    pub hk_dec: String,
    pub hk_off: String,
    pub hk_toggle: String,
    pub step_up: f32,
    pub step_down: f32,
    pub osd: OsdMode,
    pub levels: HashMap<String, f32>,
    pub mon_hk: HashMap<String, Vec<MonKey>>,
    pub mon_names: HashMap<String, String>,
    pub mon_meta: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            opacity: 0.0,
            hide: false,
            theme: ThemeMode::System,
            close_to_tray: true,
            passthrough: true,
            always_on_top: true,
            autostart: false,
            restore_opacity: true,
            hotkeys: true,
            displays: Vec::new(),
            explicit_displays: false,
            lang: Lang::default(),
            hk_inc: "Ctrl+Alt+Shift+↓".into(),
            hk_dec: "Ctrl+Alt+Shift+↑".into(),
            hk_off: "Ctrl+Alt+Shift+0".into(),
            hk_toggle: "Ctrl+Alt+Shift+N".into(),
            step_up: 5.0,
            step_down: 5.0,
            osd: OsdMode::Force,
            levels: HashMap::new(),
            mon_hk: HashMap::new(),
            mon_names: HashMap::new(),
            mon_meta: HashMap::new(),
        }
    }
}

impl Config {
    pub fn mon_entries(&self, device: &str) -> Vec<MonKey> {
        self.mon_hk.get(device).cloned().unwrap_or_default()
    }

    pub fn set_mon_entries(&mut self, device: &str, list: Vec<MonKey>) {
        if list.is_empty() {
            self.mon_hk.remove(device);
        } else {
            self.mon_hk.insert(device.to_string(), list);
        }
    }

    pub fn dir() -> PathBuf {
        if let Some(custom) = std::env::var_os("NV_CONFIG_DIR") {
            return PathBuf::from(custom);
        }
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        {
            let probe = dir.join(".nv_write_probe");
            if std::fs::write(&probe, b"1").is_ok() {
                let _ = std::fs::remove_file(&probe);
                return dir;
            }
        }
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("NightViewing")
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.json")
    }

    pub fn load() -> Self {
        let mut cfg: Self = std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        cfg.opacity = cfg.opacity.clamp(0.0, MAX_LEVEL);
        cfg.step_up = cfg.step_up.clamp(0.0, 100.0);
        cfg.step_down = cfg.step_down.clamp(0.0, 100.0);
        for v in cfg.levels.values_mut() {
            *v = v.clamp(0.0, MAX_LEVEL);
        }
        if !cfg.restore_opacity {
            cfg.opacity = Self::default().opacity;
        }
        cfg.hk_inc = crate::hotkey::normalize(&cfg.hk_inc);
        cfg.hk_dec = crate::hotkey::normalize(&cfg.hk_dec);
        cfg.hk_off = crate::hotkey::normalize(&cfg.hk_off);
        cfg.hk_toggle = crate::hotkey::normalize(&cfg.hk_toggle);
        for list in cfg.mon_hk.values_mut() {
            for e in list.iter_mut() {
                e.spec = crate::hotkey::normalize(&e.spec);
            }
        }
        cfg
    }

    pub fn save(&self) {
        let dir = Self::dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), text);
        }
    }

    pub fn level_for(&self, device: &str) -> f32 {
        self.levels
            .get(device)
            .copied()
            .unwrap_or(self.opacity)
            .clamp(0.0, MAX_LEVEL)
    }

    pub fn display_on(&self, device: &str) -> bool {
        if !self.explicit_displays {
            return true;
        }
        self.displays.iter().any(|d| d == device)
    }
}
