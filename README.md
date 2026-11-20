# Night Viewing · 夜览

> 一个用纯黑遮罩把屏幕压暗的 Windows 小工具 —— 不调背光，只叠一层可调透明度的黑色，任何窗口、任何场景都能降亮度。

[![License: LGPL-2.1](https://img.shields.io/badge/License-LGPL--2.1-blue.svg)](https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html)
![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011%20x64-0078D4)
![Rust](https://img.shields.io/badge/built%20with-Rust-orange)

---

## 这是什么

很多笔记本/显示器的最低亮度在夜里依然刺眼，而显示器本身的亮度调节又常常线性很差。**Night Viewing** 的做法是在所有窗口之上铺一层 `#000000` 的遮罩窗口，用调节透明度（0% ~ 90%）的方式等效降低屏幕亮度，不修改任何系统或驱动设置。

与传统“护眼软件”的区别：

- **只做减法**：颜色是纯黑，不偏黄、不偏红，不影响任何色彩还原
- **多显示器各自独立**：每块屏可以单独开关、单独亮度、单独快捷键
- **极轻量**：原生 Win32 自绘界面，没有 Electron、没有 WebView；峰值内存约 2 MB 私有字节
- **不打扰**：默认在托盘后台运行，截屏时遮罩不会出现在截图里

## 功能

### 亮度遮罩

| 功能 | 说明 |
| --- | --- |
| 透明度调节 | 0%（完全透明）~ 90%（最暗），滑块拖动 / 点击 / 键盘 ← → 微调（Shift 加速） |
| 数值输入 | 右上角输入框可直接键入 0 ~ 90 的任意整数 |
| 快捷档位 | 柔光 / 适中 / 深暗 / 极暗 一键切换 |
| 任意分辨率 | 通过 `EnumDisplayMonitors` 取所有显示器的并集，4K / 8K / 混合 DPI / 负坐标副屏都能完整覆盖 |
| 强制覆盖 | 遮罩始终位于 `WS_EX_TOPMOST`，独占全屏的应用和游戏也压不住它 |

### 多显示器

- 显示器列表自动枚举，显示真实的显示器名称（如 `RTK UHD HDR`）与分辨率，主屏带徽标
- 每块屏的遮罩可以**单独关闭**（关闭后该屏完全不叠加任何东西）
- 悬浮提示与托盘面板里有「全部显示器」总控行，一次同步所有屏；下面是每块屏的独立滑块

### 快捷键

默认全局快捷键（可在设置中改绑，冲突时自动换备用键位）：

| 行为 | 快捷键 | 备用 |
| --- | --- | --- |
| 遮罩加深 | `Ctrl+Alt+Shift+↑` | `Ctrl+Alt+PageUp` |
| 遮罩减淡 | `Ctrl+Alt+Shift+↓` | `Ctrl+Alt+PageDown` |
| 关闭遮罩（0%） | `Ctrl+Alt+Shift+0` | — |
| 显示主界面 | `Ctrl+Alt+Shift+N` | — |

- **步长可调**：加深 / 减淡各有独立的 ±1% / ±2% / ±5% / ±10% 预设，也支持输入任意值
- **长按连发**：按住不放会持续增减（没有用 `MOD_NOREPEAT`）
- **每屏独立快捷键**：每个条目可绑定「加深 / 减淡 / 关闭」之一，步长也独立；一块屏可以挂多条
- **冲突检测**：任何两个快捷键重复时，该输入框变红并在下方提示与哪个条目冲突

### 悬浮提示与快捷面板

- 按快捷键时，在任务栏正上方弹出一条**临时浮层**，显示每块屏当前亮度
- 单击托盘图标弹出**快捷调节面板**，同样一条总控 + 每屏一条
- 两者都支持**直接拖动滑块**（浮层未消失时也能拖）
- 层级永远在遮罩之上，点击空白处自动消失
- 三档行为可选：**强制显示**（独占全屏也显示）/ **最大兼容**（仅窗口化）/ **禁用**

### 外观与本地化

- 主题：跟随系统 / 深色 / 浅色（同时联动标题栏的 Windows 深色模式与图标颜色）
- 界面语言：简体中文、繁體中文、English (US)、English (UK)、日本語、한국어、Français、Deutsch、Español、Italiano、Русский、Português (BR)、Polski、Nederlands、Türkçe、Українська、Tiếng Việt、Bahasa Indonesia、ไทย、हिन्दी
- 语言切换会同时改变界面、托盘菜单、窗口标题与右键属性里的软件名称

### 其他

- **关闭到托盘** 或 **直接退出**，可在设置中切换
- **开机自启**（写入当前用户的启动项，无需管理员权限）
- **鼠标穿透**：遮罩不拦截任何点击
- **截图排除**：使用 `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`，Snipaste / PixPin / QQ / 微信 / 系统截图都拍不到遮罩
- **单实例**：重复启动会唤起已有窗口
- **便携**：配置写在 exe 同目录的 `config.json`，不写注册表，删掉即走

## 下载与使用

1. 到 [Releases](../../releases) 下载 `NightViewing.exe`
2. 放到任意目录双击运行（无需安装、无需管理员权限）
3. 首次启动只在托盘出现一个太阳图标 —— 单击托盘图标打开界面

> 首次运行会在 exe 同目录生成 `config.json`；若该目录不可写（例如放在 Program Files），会改用 `%APPDATA%\NightViewing\config.json`。

## 快捷键与托盘

- **单击托盘图标**：打开右下角快捷调节面板
- **双击托盘图标**：打开主界面
- **右键托盘图标**：显示主界面 / 加深 / 减淡 / 关闭遮罩 / 开机自启 / 退出

## 从源码构建

需要 Rust 1.80+ 与 MSVC 或 GNU 工具链（本项目在 Windows 上用 `rust-lld` 链接）：

```bash
git clone https://github.com/TorinLinsley/night-viewing.git
cd night-viewing
cargo build --release
# 产物：target/release/night_viewing.exe
```

图标与版本信息资源通过 `build.rs` 调用 `rc.exe` 编译 `assets/app.rc`；若环境里找不到资源编译器，会跳过该步（程序仍可编译运行，只是 exe 没有图标与版本信息）。可以用环境变量 `RC` 指定 `rc.exe` 的绝对路径。

## 实现要点

- **纯原生 Win32**：界面是 GDI 自绘（双缓冲 `CreateCompatibleDC` + `BitBlt`），没有引入任何 GUI 框架
- **分层窗口**：每块显示器一个 `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` 的弹出窗口，用 `SetLayeredWindowAttributes` 调 alpha
- **DPI**：进程启动即 `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`，并用 `GetDpiForWindow` 适配 100% ~ 200% 缩放
- **热键**：`RegisterHotKey` 挂在消息专用窗口上，注册失败自动降级到备用键位，并在日志中记录原因
- **内存**：无 GPU 渲染、无 WebView，release 构建开启 `lto`、`opt-level = "s"`、`panic = "abort"`、`strip`

## 已知限制

- 仅支持 Windows 10 / 11 x64（依赖分层窗口与 `WDA_EXCLUDEFROMCAPTURE`）
- 遮罩层无法覆盖 UAC 安全桌面、Ctrl+Alt+Del 界面与登录界面
- 部分显卡驱动 / 厂商工具会抢占 `Ctrl+Alt+方向键`，此时请使用 PageUp / PageDown 备用键位或自行改绑
- 不修改显示器硬件亮度，因此对“黑色本身发光”的 OLED 屏效果优于对背光常亮的 IPS 屏

## 许可证

本项目基于 **GNU Lesser General Public License v2.1** 发布，详见 [LICENSE](LICENSE)。

```
Copyright (C) 2026 Torin Linsley
```

## English

**Night Viewing** is a tiny Windows tray utility that dims your screen by overlaying a pure `#000000` layered window whose opacity you control (0–90%), instead of touching your monitor's backlight.

- Per-monitor overlays — enable/disable and set brightness per display, with an "All displays" row for syncing
- Hotkeys: `Ctrl+Alt+Shift+↑/↓` for darker/lighter (falls back to `PageUp/PageDown` when taken), `Ctrl+Alt+Shift+0` for off, `Ctrl+Alt+Shift+N` to show the window; hold to repeat
- Per-display hotkeys with their own actions and step sizes, plus duplicate-hotkey detection
- Transient on-screen hint above the taskbar and a tray quick panel — both with draggable sliders, both above the overlay
- Hide-on-close to tray, run at startup, click-through overlay, always-on-top (covers exclusive fullscreen)
- Excluded from screen capture, so screenshots stay clean
- 20 UI languages, light/dark/system theme, single instance, portable `config.json`
- Native Win32 + GDI, ~2 MB private working set

Requires Windows 10/11 x64. Build with `cargo build --release`. Licensed under LGPL-2.1.
