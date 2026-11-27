// 程序入口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod core;
mod gui;
mod hotkey;
mod i18n;
mod icon;
mod osd;
mod overlay;
mod tray;
mod winapi_util;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--export-ico") {
        if let Some(p) = args.get(i + 1) {
            let _ = icon::write_ico(std::path::Path::new(p));
        }
        return;
    }

    winapi_util::set_dpi_aware();
    let _instance_mutex = unsafe {
        use windows::core::PCWSTR;
        use windows::Win32::System::Threading::CreateMutexW;
        let name: Vec<u16> = "TorinLinsley.NightViewing.Instance\0".encode_utf16().collect();
        CreateMutexW(None, false, PCWSTR(name.as_ptr())).ok()
    };
    unsafe {
        use windows::Win32::Foundation::GetLastError;
        if GetLastError().0 == 183 {
            use windows::core::PCWSTR;
            use windows::Win32::UI::WindowsAndMessaging::{
                FindWindowW, SetForegroundWindow, ShowWindow, SW_SHOWNORMAL,
            };
            let cls: Vec<u16> = "NightViewing::Main\0".encode_utf16().collect();
            if let Ok(hwnd) = FindWindowW(PCWSTR(cls.as_ptr()), PCWSTR::null()) {
                let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
                let _ = SetForegroundWindow(hwnd);
            }
            return;
        }
    }
    unsafe {
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        let id: Vec<u16> = "TorinLinsley.NightViewing\0".encode_utf16().collect();
        let _ = SetCurrentProcessExplicitAppUserModelID(PCWSTR(id.as_ptr()));
    }

    let first_run = !config::Config::path().exists();
    let core = core::Core::new();
    if first_run {
        core.snapshot().0.save();
    }
    overlay::spawn(core.clone());

    let (cfg, _) = core.snapshot();
    let for_dark = match cfg.theme {
        config::ThemeMode::Dark => true,
        config::ThemeMode::Light => false,
        config::ThemeMode::System => winapi_util::system_prefers_dark(),
    };

    osd::init(&core, for_dark);
    let tray = tray::init(&core, for_dark, cfg.lang);
    let _ = std::thread::Builder::new()
        .name("night-viewing-trim".into())
        .stack_size(64 * 1024)
        .spawn(|| loop {
            winapi_util::trim_memory();
            std::thread::sleep(std::time::Duration::from_secs(10));
        });
    gui::run(core, for_dark, tray, first_run);
}
