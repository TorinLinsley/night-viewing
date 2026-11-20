// 系统托盘图标与菜单
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::core::Core;
use crate::i18n::{tr, Lang};
use crate::icon;
use crate::osd;

static CLICK_GEN: AtomicU64 = AtomicU64::new(0);
static TRAY_ORIGIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn take_tray_origin() -> bool {
    TRAY_ORIGIN.swap(false, Ordering::SeqCst)
}


pub fn init(core: &Arc<Core>, for_dark: bool, lang: Lang) -> Option<TrayIcon> {
    let menu = Menu::new();
    let show_item = MenuItem::with_id("show", tr(lang, "显示主界面"), true, None);
    let inc_item = MenuItem::with_id("inc", tr(lang, "遮罩加深  +5%"), true, None);
    let dec_item = MenuItem::with_id("dec", tr(lang, "遮罩减淡  -5%"), true, None);
    let reset_item = MenuItem::with_id("reset", tr(lang, "关闭遮罩（0%）"), true, None);
    let auto_item = CheckMenuItem::with_id("autostart", tr(lang, "开机自启"), true, false, None);
    let quit_item = MenuItem::with_id("quit", format!("{} {}", tr(lang, "退出程序"), crate::i18n::app_name(lang)), true, None);
    let sep1 = PredefinedMenuItem::separator();
    let sep2 = PredefinedMenuItem::separator();

    if menu
        .append_items(&[
            &show_item,
            &sep1,
            &inc_item,
            &dec_item,
            &reset_item,
            &sep2,
            &auto_item,
            &quit_item,
        ])
        .is_err()
    {
        return None;
    }

    let (cfg, _) = core.snapshot();
    auto_item.set_checked(cfg.autostart);

    let mut builder = TrayIconBuilder::new()
        .with_tooltip(crate::i18n::app_name(lang))
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false);
    if let Some(icon) = icon::tray_icon(for_dark) {
        builder = builder.with_icon(icon);
    }
    let tray = builder.build().ok()?;

    let core_for_click = core.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => {
            let ticket = CLICK_GEN.fetch_add(1, Ordering::SeqCst) + 1;
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(260));
                if CLICK_GEN.load(Ordering::SeqCst) == ticket {
                    osd::toggle_panel();
                }
            });
        }
        TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } => {
            CLICK_GEN.fetch_add(1, Ordering::SeqCst);
            osd::hide_panel();
            core_for_click.show_window();
        }
        _ => {}
    }));

    let core_for_menu = core.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let id = event.id().as_ref().to_string();
        match id.as_str() {
            "show" => core_for_menu.show_window(),
            "inc" => core_for_menu.nudge_step(1.0),
            "dec" => core_for_menu.nudge_step(-1.0),
            "reset" => core_for_menu.set_level(None, 0.0),
            "autostart" => {
                TRAY_ORIGIN.store(true, Ordering::SeqCst);
                core_for_menu.toggle_autostart();
            }
            "quit" => core_for_menu.quit(),
            _ => {}
        }
    }));

    Some(tray)
}
