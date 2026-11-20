// 主界面（egui）
use std::sync::Arc;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, Vec2, pos2,
    vec2,
};

use crate::config::{Config, MAX_LEVEL, ThemeMode};
use crate::core::Core;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct NvApp {
    core: Arc<Core>,
    cfg: Config,
    rev: u64,
    applied_theme: Option<egui::ThemePreference>,
    tray: Option<tray_icon::TrayIcon>,
    icon_for_dark: bool,
    num_text: String,
    num_focus: bool,
}

impl NvApp {
    pub fn new(cc: &eframe::CreationContext<'_>, core: Arc<Core>) -> Self {
        install_style(&cc.egui_ctx);
        core.attach(cc.egui_ctx.clone());
        let (cfg, rev) = core.snapshot();

        let icon_for_dark = match cfg.theme {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => crate::winapi_util::system_prefers_dark(),
        };
        let tray = crate::tray::init(&core, icon_for_dark);

        let num_text = format!("{:.0}", cfg.opacity);

        Self {
            core,
            cfg,
            rev,
            applied_theme: None,
            tray,
            icon_for_dark,
            num_text,
            num_focus: false,
        }
    }

    fn sync_icon(&mut self, ctx: &egui::Context) {
        let want_dark = ctx.theme() == egui::Theme::Dark;
        if want_dark == self.icon_for_dark {
            return;
        }
        self.icon_for_dark = want_dark;
        if let Some(tray) = &self.tray {
            let _ = tray.set_icon(crate::icon::tray_icon(want_dark));
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(crate::icon::window_icon(
            want_dark,
        ))));
    }

    fn push(&mut self, f: impl FnOnce(&mut Config)) {
        self.core.update(f);
        let (cfg, rev) = self.core.snapshot();
        self.cfg = cfg;
        self.rev = rev;
    }

    fn sync(&mut self, ctx: &egui::Context) {
        let pref = match self.cfg.theme {
            ThemeMode::System => egui::ThemePreference::System,
            ThemeMode::Dark => egui::ThemePreference::Dark,
            ThemeMode::Light => egui::ThemePreference::Light,
        };
        if self.applied_theme != Some(pref) {
            ctx.set_theme(pref);
            self.applied_theme = Some(pref);
        }
        self.sync_icon(ctx);

        let (cfg, rev) = self.core.snapshot();
        if rev != self.rev {
            self.cfg = cfg;
            self.rev = rev;
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.core.quitting() {
            if self.cfg.close_to_tray {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.core.hide_window();
            }
        }

        if self.core.flush_due() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let bg = ui.visuals().panel_fill;
        ui.painter().rect_filled(ui.max_rect(), CornerRadius::ZERO, bg);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(6.0);
                self.header(ui);
                ui.add_space(14.0);
                self.dim_card(ui);
                ui.add_space(12.0);
                let mut cfg = self.cfg.clone();
                let mut changed = false;

                changed |= self.appearance_card(ui, &mut cfg);
                ui.add_space(12.0);
                changed |= self.behavior_card(ui, &mut cfg);
                ui.add_space(12.0);
                changed |= self.overlay_card(ui, &mut cfg);

                if changed {
                    self.push(|c| *c = cfg);
                }

                ui.add_space(14.0);
                self.footer(ui);
                ui.add_space(8.0);
            });
    }

    fn header(&self, ui: &mut egui::Ui) {
        let accent = accent_of(ui);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            let c = rect.center();
            let p = ui.painter();
            let stroke = Stroke::new(2.0, accent);
            p.circle_stroke(c, 8.0, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (s, co) = a.sin_cos();
                p.line_segment(
                    [
                        pos2(c.x + co * 12.0, c.y + s * 12.0),
                        pos2(c.x + co * 16.5, c.y + s * 16.5),
                    ],
                    stroke,
                );
            }

            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new("Night Viewing").size(20.0).strong());
                ui.label(
                    RichText::new("夜间护眼 · 纯黑遮罩调光")
                        .size(11.5)
                        .color(dim(ui.visuals().text_color(), 0.55)),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let level = self.cfg.opacity;
                let text = if level <= 0.5 {
                    "遮罩已关闭".to_string()
                } else {
                    format!("亮度 ≈ {:.0}%", 100.0 - level)
                };
                let (rect, _) = ui.allocate_exact_size(
                    vec2(text_width(ui, &text, 11.5) + 20.0, 24.0),
                    Sense::hover(),
                );
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(12), dim(accent, 0.14));
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(11.5),
                    accent,
                );
            });
        });
    }

    fn dim_card(&mut self, ui: &mut egui::Ui) {
        let mut level = self.cfg.opacity;
        let mut changed = false;
        let mut num_focus = self.num_focus;
        let mut num_text = if self.num_focus {
            self.num_text.clone()
        } else {
            format!("{:.0}", level)
        };
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("遮罩不透明度")
                        .size(12.0)
                        .strong()
                        .color(dim(ui.visuals().text_color(), 0.7)),
                );
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new("0% = 透明 · 90% = 最暗")
                            .size(11.0)
                            .color(dim(ui.visuals().text_color(), 0.45)),
                    );
                });
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    RichText::new(format!("{:.0}", level))
                        .size(44.0)
                        .strong()
                        .color(ui.visuals().text_color()),
                );
                ui.add_space(1.0);
                ui.label(
                    RichText::new("%")
                        .size(15.0)
                        .color(dim(ui.visuals().text_color(), 0.5)),
                );

                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new("%")
                            .size(12.0)
                            .color(dim(ui.visuals().text_color(), 0.45)),
                    );
                    let edit = egui::TextEdit::singleline(&mut num_text)
                        .desired_width(34.0)
                        .char_limit(3)
                        .horizontal_align(Align::RIGHT)
                        .font(FontId::proportional(13.5))
                        .margin(egui::Margin::symmetric(6, 4));
                    let resp = ui.add(edit);

                    if resp.has_focus() {
                        num_focus = true;
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Some(v) = parse_level(&num_text) {
                                level = v;
                                changed = true;
                            }
                            num_text = format!("{:.0}", level);
                            ui.memory_mut(|m| m.surrender_focus(resp.id));
                            num_focus = false;
                        }
                    } else if num_focus {
                        if let Some(v) = parse_level(&num_text) {
                            if (v - level).abs() > 0.5 {
                                level = v;
                                changed = true;
                            }
                        }
                        num_text = format!("{:.0}", level);
                        num_focus = false;
                    }

                    ui.label(
                        RichText::new("自定义")
                            .size(11.5)
                            .color(dim(ui.visuals().text_color(), 0.45)),
                    );
                });
            });

            ui.add_space(2.0);
            if dim_slider(ui, &mut level).changed() {
                changed = true;
            }
            ui.label(
                RichText::new("拖动或点击滑块；按 Tab 聚焦滑块后用 ← → 微调（Shift 加速）")
                    .size(10.5)
                    .color(dim(ui.visuals().text_color(), 0.4)),
            );
            ui.add_space(2.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (label, value) in [
                    ("柔光", 20.0f32),
                    ("适中", 40.0),
                    ("深暗", 65.0),
                    ("极暗", 90.0),
                ] {
                    let selected = (self.cfg.opacity - value).abs() < 0.5;
                    if chip(ui, label, selected).clicked() {
                        level = value;
                        changed = true;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let text = format!("遮罩 {:.0}%", level);
                    ui.label(
                        RichText::new(text)
                            .size(11.0)
                            .color(dim(ui.visuals().text_color(), 0.45)),
                    );
                });
            });
        });
        self.num_text = num_text;
        self.num_focus = num_focus;
        if changed {
            self.push(|c| c.opacity = level);
        }
    }

    fn appearance_card(&mut self, ui: &mut egui::Ui, cfg: &mut Config) -> bool {
        let mut changed = false;
        card(ui, |ui| {
            section(ui, "外观");
            ui.add_space(6.0);
            row(ui, "主题", Some("自动跟随 Windows 的浅色 / 深色模式"), |_ui| {});
            ui.add_space(4.0);
            let mut idx = match cfg.theme {
                ThemeMode::System => 0usize,
                ThemeMode::Dark => 1,
                ThemeMode::Light => 2,
            };
            if segmented(ui, &mut idx, &["跟随系统", "深色", "浅色"]) {
                cfg.theme = match idx {
                    1 => ThemeMode::Dark,
                    2 => ThemeMode::Light,
                    _ => ThemeMode::System,
                };
                changed = true;
            }
        });
        changed
    }

    fn behavior_card(&mut self, ui: &mut egui::Ui, cfg: &mut Config) -> bool {
        let mut changed = false;
        card(ui, |ui| {
            section(ui, "行为");
            ui.add_space(6.0);

            row(ui, "关闭窗口时最小化到托盘", Some("关闭后仍在后台运行，可从托盘恢复"), |ui| {
                if toggle(ui, &mut cfg.close_to_tray).changed() {
                    changed = true;
                }
            });
            ui.add_space(8.0);
            row(ui, "开机自动启动", Some("登录 Windows 后自动在托盘运行"), |ui| {
                if toggle(ui, &mut cfg.autostart).changed() {
                    changed = true;
                }
            });
            ui.add_space(8.0);
            row(ui, "启动时恢复上次强度", Some("关闭则每次以 25% 的柔和遮罩启动"), |ui| {
                if toggle(ui, &mut cfg.restore_opacity).changed() {
                    changed = true;
                }
            });
            ui.add_space(8.0);
            row(
                ui,
                "全局快捷键",
                Some("Ctrl+Alt+↑ / ↓ 调光 · Ctrl+Alt+0 关闭 · Ctrl+Alt+N 显示窗口（若 ↑ / ↓ 被占用则自动改用 Ctrl+Alt+Shift+↑ / ↓）"),
                |ui| {
                    if toggle(ui, &mut cfg.hotkeys).changed() {
                        changed = true;
                    }
                },
            );
        });
        changed
    }

    fn overlay_card(&mut self, ui: &mut egui::Ui, cfg: &mut Config) -> bool {
        let mut changed = false;
        card(ui, |ui| {
            section(ui, "遮罩");
            ui.add_space(6.0);
            row(ui, "鼠标穿透", Some("遮罩不拦截点击，可正常操作电脑"), |ui| {
                if toggle(ui, &mut cfg.passthrough).changed() {
                    changed = true;
                }
            });
            ui.add_space(8.0);
            row(ui, "始终置顶", Some("覆盖在其他窗口（包括全屏应用）之上"), |ui| {
                if toggle(ui, &mut cfg.always_on_top).changed() {
                    changed = true;
                }
            });
        });
        changed
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("v{VERSION}"))
                    .size(11.0)
                    .color(dim(ui.visuals().text_color(), 0.4)),
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let btn = egui::Button::new(RichText::new("退出程序").size(12.0).color(
                    ui.visuals().text_color(),
                ))
                .fill(dim(ui.visuals().text_color(), 0.08))
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::same(9))
                .min_size(vec2(78.0, 28.0));
                if ui.add(btn).clicked() {
                    self.core.quit();
                }
                if ui.button("隐藏到托盘").clicked() {
                    self.core.hide_window();
                }
            });
        });
    }
}

impl eframe::App for NvApp {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.sync(ui.ctx());
        self.draw(ui);
    }
}

fn accent_of(ui: &egui::Ui) -> Color32 {
    ui.visuals().selection.bg_fill
}

fn dim(c: Color32, f: f32) -> Color32 {
    c.gamma_multiply(f)
}

fn text_width(ui: &egui::Ui, text: &str, size: f32) -> f32 {
    ui.fonts_mut(|f| {
        f.layout_no_wrap(
            text.to_owned(),
            FontId::proportional(size),
            Color32::WHITE,
        )
        .size()
        .x
    })
}

fn install_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|st| {
        let dark = st.visuals.dark_mode;
        st.visuals = make_visuals(dark);
        st.spacing.item_spacing = vec2(8.0, 8.0);
        st.spacing.button_padding = vec2(12.0, 6.0);
        st.spacing.interact_size.y = 26.0;
        st.text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(13.5));
        st.text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        st.text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(11.0));
        st.text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(19.0));
    });
}

fn make_visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let accent = if dark {
        Color32::from_rgb(140, 152, 255)
    } else {
        Color32::from_rgb(76, 92, 214)
    };
    let text = if dark {
        Color32::from_rgb(228, 231, 240)
    } else {
        Color32::from_rgb(28, 32, 42)
    };

    v.panel_fill = if dark {
        Color32::from_rgb(15, 16, 21)
    } else {
        Color32::from_rgb(245, 246, 251)
    };
    v.window_fill = if dark {
        Color32::from_rgb(20, 22, 28)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    v.extreme_bg_color = if dark {
        Color32::from_rgb(11, 12, 16)
    } else {
        Color32::from_rgb(236, 238, 246)
    };
    v.faint_bg_color = if dark {
        Color32::from_rgb(24, 26, 33)
    } else {
        Color32::from_rgb(252, 252, 255)
    };
    v.window_stroke = Stroke::new(
        1.0,
        if dark {
            Color32::from_rgb(38, 41, 51)
        } else {
            Color32::from_rgb(226, 229, 238)
        },
    );
    v.hyperlink_color = accent;
    v.override_text_color = Some(text);
    v.selection = egui::style::Selection {
        bg_fill: accent,
        stroke: Stroke::new(1.0, Color32::WHITE),
    };

    let widget_fill = if dark {
        Color32::from_rgb(30, 33, 41)
    } else {
        Color32::from_rgb(232, 235, 243)
    };
    v.widgets.noninteractive.bg_fill = v.panel_fill;
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text);
    v.widgets.noninteractive.corner_radius = CornerRadius::same(10);

    v.widgets.inactive.bg_fill = widget_fill;
    v.widgets.inactive.weak_bg_fill = widget_fill;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, text);
    v.widgets.inactive.corner_radius = CornerRadius::same(10);

    v.widgets.hovered.bg_fill = dim(widget_fill, 1.35);
    v.widgets.hovered.weak_bg_fill = dim(widget_fill, 1.35);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, dim(accent, 0.6));
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, text);
    v.widgets.hovered.corner_radius = CornerRadius::same(10);

    v.widgets.active.bg_fill = dim(accent, 0.35);
    v.widgets.active.weak_bg_fill = dim(accent, 0.35);
    v.widgets.active.bg_stroke = Stroke::new(1.0, accent);
    v.widgets.active.fg_stroke = Stroke::new(1.5, text);
    v.widgets.active.corner_radius = CornerRadius::same(10);

    v
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let dark = ui.visuals().dark_mode;
    let fill = if dark {
        Color32::from_rgb(23, 25, 32)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    let stroke = Stroke::new(
        1.0,
        if dark {
            Color32::from_rgb(36, 39, 49)
        } else {
            Color32::from_rgb(228, 231, 240)
        },
    );
    egui::Frame::NONE
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add(ui)
        })
        .inner
}

fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(11.0)
            .strong()
            .color(dim(ui.visuals().text_color(), 0.4)),
    );
}

fn row(
    ui: &mut egui::Ui,
    title: &str,
    desc: Option<&str>,
    right: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(title).size(13.0));
            if let Some(d) = desc {
                ui.label(
                    RichText::new(d)
                        .size(10.8)
                        .color(dim(ui.visuals().text_color(), 0.45)),
                );
            }
        });
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            right(ui);
        });
    });
}

fn toggle(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(44.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.11);
    let accent = accent_of(ui);
    let off = if ui.visuals().dark_mode {
        Color32::from_rgb(55, 59, 71)
    } else {
        Color32::from_rgb(203, 208, 220)
    };
    let bg = lerp_color(off, accent, t);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(12), bg);
    let r = 8.5;
    let x = egui::lerp((rect.left() + r + 2.5)..=(rect.right() - r - 2.5), t);
    p.circle_filled(pos2(x, rect.center().y), r, Color32::from_rgb(255, 255, 255));
    resp
}

fn parse_level(text: &str) -> Option<f32> {
    text.trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f32>()
        .ok()
        .map(|v| v.clamp(0.0, MAX_LEVEL))
}

fn dim_slider(ui: &mut egui::Ui, value: &mut f32) -> egui::Response {
    let width = ui.available_width();
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click_and_drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let track = Rect::from_center_size(rect.center(), vec2(width, 8.0));
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = ui.ctx().pointer_interact_pos() {
            resp.request_focus();
            let frac = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
            let new_value = frac * MAX_LEVEL;
            if (new_value - *value).abs() > 0.01 {
                *value = new_value;
                resp.mark_changed();
            }
        }
    }

    if resp.has_focus() {
        let step = if ui.input(|i| i.modifiers.shift) {
            10.0
        } else {
            1.0
        };
        let mut delta = 0.0;
        ui.input(|i| {
            if i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowDown) {
                delta -= step;
            }
            if i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowUp) {
                delta += step;
            }
            if i.key_pressed(egui::Key::PageDown) {
                delta -= 10.0;
            }
            if i.key_pressed(egui::Key::PageUp) {
                delta += 10.0;
            }
            if i.key_pressed(egui::Key::Home) {
                delta = -*value;
            }
            if i.key_pressed(egui::Key::End) {
                delta = MAX_LEVEL - *value;
            }
        });
        if delta != 0.0 {
            let new_value = (*value + delta).clamp(0.0, MAX_LEVEL);
            if (new_value - *value).abs() > 0.01 {
                *value = new_value;
                resp.mark_changed();
            } else {
                ui.ctx().request_repaint();
            }
        }
    }
    let frac = (*value / MAX_LEVEL).clamp(0.0, 1.0);
    let accent = accent_of(ui);
    let dark = ui.visuals().dark_mode;
    let track_bg = if dark {
        Color32::from_rgb(37, 40, 50)
    } else {
        Color32::from_rgb(226, 229, 238)
    };
    let p = ui.painter();
    if resp.has_focus() {
        p.rect_stroke(
            track.expand(3.0),
            CornerRadius::same(7),
            Stroke::new(1.5, dim(accent, 0.55)),
            egui::StrokeKind::Middle,
        );
    }
    p.rect_filled(track, CornerRadius::same(4), track_bg);
    if frac > 0.0 {
        let filled = Rect::from_min_max(
            track.min,
            pos2(track.left() + track.width() * frac, track.bottom()),
        );
        p.rect_filled(filled, CornerRadius::same(4), accent);
    }
    let knob = pos2(track.left() + track.width() * frac, track.center().y);
    let active = resp.hovered() || resp.dragged() || resp.has_focus();
    if active {
        p.circle_filled(knob, 13.0, dim(accent, 0.22));
    }
    p.circle_filled(knob, 9.0, Color32::from_rgb(255, 255, 255));
    p.circle_stroke(knob, 9.0, Stroke::new(2.0, accent));
    resp
}

fn chip(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let accent = accent_of(ui);
    let dark = ui.visuals().dark_mode;
    let text = ui.visuals().text_color();
    let (fill, fg) = if selected {
        (
            dim(accent, if dark { 0.30 } else { 0.16 }),
            if dark { accent } else { accent },
        )
    } else {
        (
            dim(text, if dark { 0.08 } else { 0.05 }),
            dim(text, 0.7),
        )
    };
    let btn = egui::Button::new(RichText::new(label).size(12.0).color(fg))
        .fill(fill)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(9))
        .min_size(vec2(56.0, 28.0));
    let resp = ui.add(btn);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

fn segmented(ui: &mut egui::Ui, selected: &mut usize, labels: &[&str]) -> bool {
    let n = labels.len().max(1);
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 34.0), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let dark = ui.visuals().dark_mode;
    let accent = accent_of(ui);
    let bg = if dark {
        Color32::from_rgb(16, 18, 24)
    } else {
        Color32::from_rgb(237, 239, 246)
    };
    let seg_w = rect.width() / n as f32;
    let mut clicked = None;
    if resp.clicked() {
        if let Some(pos) = ui.ctx().pointer_interact_pos() {
            let i = (((pos.x - rect.left()) / seg_w).floor() as usize).min(n - 1);
            clicked = Some(i);
        }
    }
    let anim = ui
        .ctx()
        .animate_value_with_time(resp.id.with("highlight"), *selected as f32, 0.13);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(11), bg);
    let hl = Rect::from_min_size(
        pos2(rect.left() + seg_w * anim + 3.0, rect.top() + 3.0),
        vec2(seg_w - 6.0, rect.height() - 6.0),
    );
    p.rect_filled(hl, CornerRadius::same(9), dim(accent, if dark { 0.30 } else { 0.16 }));
    for (i, label) in labels.iter().enumerate() {
        let center = pos2(rect.left() + seg_w * (i as f32 + 0.5), rect.center().y);
        let color = if i == *selected {
            accent
        } else {
            dim(ui.visuals().text_color(), 0.6)
        };
        p.text(
            center,
            Align2::CENTER_CENTER,
            *label,
            FontId::proportional(12.5),
            color,
        );
    }
    if let Some(i) = clicked {
        *selected = i;
        true
    } else {
        false
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

#[allow(dead_code)]
fn unused(_: Vec2, _: Align) {}
