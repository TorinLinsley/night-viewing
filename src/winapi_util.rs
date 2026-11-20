// 系统交互：开机自启、系统明暗主题、显示器枚举
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, GetDC, GetDeviceCaps,
    GetMonitorInfoW, HDC, HMONITOR, LOGPIXELSX, MONITORINFO, MONITORINFOEXW, ReleaseDC,
};
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    MONITORINFOF_PRIMARY, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey,
    RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "NightViewing";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn system_prefers_dark() -> bool {
    const PERSONALIZE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
    let sub = wide(PERSONALIZE);
    let name = wide("AppsUseLightTheme");
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(sub.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if ok.is_ok() { data == 0 } else { true }
}

pub fn work_area() -> RECT {
    let mut rc = RECT::default();
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rc as *mut RECT as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    rc
}

pub fn system_locale() -> String {
    const INTL: &str = "Control Panel\\International";
    let sub = wide(INTL);
    let name = wide("LocaleName");
    let mut buf = [0u16; 96];
    let mut size = (buf.len() * 2) as u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(sub.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if ok.is_ok() {
        String::from_utf16_lossy(&buf)
            .trim_end_matches('\0')
            .to_string()
    } else {
        String::new()
    }
}

pub fn set_dpi_aware() {
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
}

pub fn system_scale() -> f32 {
    unsafe {
        let dc = GetDC(None);
        if dc.0.is_null() {
            return 1.0;
        }
        let dpi = GetDeviceCaps(Some(dc), LOGPIXELSX);
        let _ = ReleaseDC(None, dc);
        if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 }
    }
}

fn open_run_key() -> Option<HKEY> {
    let path = wide(RUN_KEY);
    let mut key = HKEY::default();
    let err = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            Some(0),
            KEY_SET_VALUE,
            &mut key,
        )
    };
    if err.is_ok() { Some(key) } else { None }
}

pub fn set_autostart(enable: bool) -> bool {
    let Some(key) = open_run_key() else {
        return false;
    };
    let name = wide(VALUE_NAME);
    let ok = unsafe {
        if enable {
            let Ok(exe) = std::env::current_exe().map(|p| format!("\"{}\"", p.display())) else {
                let _ = RegCloseKey(key);
                return false;
            };
            let data = wide(&exe);
            let bytes = std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
            RegSetValueExW(key, PCWSTR(name.as_ptr()), Some(0), REG_SZ, Some(bytes)).is_ok()
        } else {
            let err = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
            err.is_ok() || err.0 == 2
        }
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    ok
}

#[derive(Clone, Debug)]
pub struct Monitor {
    pub device: String,
    pub name: String,
    pub friendly: String,
    pub rect: RECT,
    pub primary: bool,
}

impl Monitor {
    pub fn width(&self) -> i32 {
        self.rect.right - self.rect.left
    }

    pub fn height(&self) -> i32 {
        self.rect.bottom - self.rect.top
    }
}

struct Collect {
    items: Vec<Monitor>,
    names: std::collections::HashMap<String, String>,
}

unsafe extern "system" fn collect(hmon: HMONITOR, _hdc: HDC, _rect: *mut RECT, data: LPARAM) -> BOOL {
    unsafe {
        let sink = &mut *(data.0 as *mut Collect);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(hmon, &mut info.monitorInfo as *mut MONITORINFO).as_bool() {
            return BOOL(1);
        }
        let raw = &info.szDevice;
        let end = raw.iter().position(|c| *c == 0).unwrap_or(raw.len());
        let device = String::from_utf16_lossy(&raw[..end]);
        let mut name = String::new();
        let mut dd = DISPLAY_DEVICEW::default();
        dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let wide_device = wide(&device);
        if EnumDisplayDevicesW(PCWSTR(wide_device.as_ptr()), 0, &mut dd, 0).as_bool() {
            let end = dd.DeviceString.iter().position(|c| *c == 0).unwrap_or(0);
            name = String::from_utf16_lossy(&dd.DeviceString[..end]);
        }
        let friendly = sink.names.get(&device).cloned().unwrap_or_default();
        sink.items.push(Monitor {
            device,
            name,
            friendly,
            rect: info.monitorInfo.rcMonitor,
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }
    BOOL(1)
}

fn friendly_names() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    unsafe {
        let mut paths = 0u32;
        let mut modes = 0u32;
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut paths, &mut modes).0 != 0 {
            return map;
        }
        let mut path_buf = vec![DISPLAYCONFIG_PATH_INFO::default(); paths as usize];
        let mut mode_buf = vec![DISPLAYCONFIG_MODE_INFO::default(); modes as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut paths,
            path_buf.as_mut_ptr(),
            &mut modes,
            mode_buf.as_mut_ptr(),
            None,
        )
        .0 != 0
        {
            return map;
        }
        for path in path_buf.iter().take(paths as usize) {
            let mut src = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            src.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            src.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            src.header.adapterId = path.sourceInfo.adapterId;
            src.header.id = path.sourceInfo.id;
            if DisplayConfigGetDeviceInfo(&mut src.header) != 0 {
                continue;
            }
            let end = src.viewGdiDeviceName.iter().position(|c| *c == 0).unwrap_or(0);
            let gdi = String::from_utf16_lossy(&src.viewGdiDeviceName[..end]);
            let mut dst = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            dst.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            dst.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            dst.header.adapterId = path.targetInfo.adapterId;
            dst.header.id = path.targetInfo.id;
            if DisplayConfigGetDeviceInfo(&mut dst.header) != 0 {
                continue;
            }
            let end = dst.monitorFriendlyDeviceName.iter().position(|c| *c == 0).unwrap_or(0);
            let friendly = String::from_utf16_lossy(&dst.monitorFriendlyDeviceName[..end]);
            if !friendly.is_empty() {
                map.insert(gdi, friendly);
            }
        }
    }
    map
}

pub fn monitors() -> Vec<Monitor> {
    let mut sink = Collect { items: Vec::new(), names: friendly_names() };
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut sink as *mut _ as isize),
        );
    }
    let mut items = sink.items;
    items.sort_by_key(|m| (m.rect.left, m.rect.top));
    items
}
