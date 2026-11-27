# Night Viewing · 夜览

有些显示器已经调到最低亮度了还不够暗。**Night Viewing** 的做法是在屏幕上铺一层纯黑色透明遮罩图层，用调节透明度（0% ~ 90%）的方式等效降低屏幕亮度

**适用平台：Winodws 10 / 11**

## 下载与使用

1. 到 [Releases](../../releases) 下载 `night_viewing_vX.X.X.zip`
2. 放到任意目录双击`NightViewing.exe`运行（无需安装、无需管理员权限）
3. 首次启动只在托盘出现一个软件图标 —— 双击托盘图标或者右键托盘图标方式打开界面

> 首次运行会在 exe 同目录生成 `config.json`；若该目录不可写（例如放在 Program Files），会改用 `%APPDATA%\NightViewing\config.json`。

## 功能

### 亮度遮罩

| 功能       | 说明                                                         |
| ---------- | ------------------------------------------------------------ |
| 透明度调节 | 0%（完全透明）~ 90%（最暗），滑块拖动 / 点击 / 键盘 ←  → 微调（Shift 加速） |
| 数值输入   | 右上角输入框可直接键入 0 ~ 90 的任意整数                     |
| 预设档位   | 柔光 / 适中 / 深暗 / 极暗 一键切换                           |
| 任意分辨率 | 通过 `EnumDisplayMonitors` 取所有显示器的并集，1080p / 4K...，混合 DPI / 负坐标副屏都能完整覆盖 |
| 强制覆盖   | 遮罩始终位于 `WS_EX_TOPMOST`，独占全屏的应用和游戏有可能覆盖不了 |

### 多显示器

- 显示器列表自动枚举，显示真实的显示器名称（如 `NE160WUM-NX2`）与分辨率，主屏带徽标
- 每块屏的遮罩可以**单独关闭**（关闭后该屏完全不叠加任何东西）
- 悬浮提示与托盘面板里有「全部显示器」总控行，一次同步所有屏；下面是每块屏的独立滑块
- 新增「连接屏幕管理」板块：已保存过独立快捷键的显示器即使暂时断开也会保留在列表里，可在这里查看其名称 / 分辨率 / 刷新率、展开修改或删除它的按键配置；当前正在连接的屏幕也可以用 × 清空该屏的按键配置，断开且没配置过的屏幕会自动消失

### 快捷键

默认全局快捷键（可在设置中改绑，冲突时自动换备用键位）：

| 行为           | 快捷键             | 备用                |
| -------------- | ------------------ | ------------------- |
| 遮罩加深       | `Ctrl+Alt+Shift+↑` | `Ctrl+Alt+PageUp`   |
| 遮罩减淡       | `Ctrl+Alt+Shift+↓` | `Ctrl+Alt+PageDown` |
| 关闭遮罩（0%） | `Ctrl+Alt+Shift+0` | —                   |
| 显示主界面     | `Ctrl+Alt+Shift+N` | —                   |

- **步长可调**：加深 / 减淡各有独立的 ±1% / ±2% / ±5% / ±10% 预设，也支持输入任意值
- **长按连发**：按住不放会持续增减（没有用 `MOD_NOREPEAT`）
- **每屏独立快捷键**：每个条目可绑定「加深 / 减淡 / 关闭」之一，步长也独立；一块屏可以挂多条
- **冲突检测**：任何两个快捷键重复时，该输入框变红并在下方提示与哪个条目冲突

### 悬浮提示与快捷面板

- 按快捷键时，屏幕右下角弹出**快捷调节面板**，显示每块屏当前亮度
- 单击托盘图标弹出**快捷调节面板**，同样一条总控 + 每屏一条
- 三档行为可选：**强制显示**（独占全屏也显示）/ **最大兼容**（仅窗口化）/ **禁用**

### 外观与本地化

- 主题：跟随系统 / 深色 / 浅色（同时联动标题栏的 Windows 深色模式与图标颜色）
- 支持的界面语言：简体中文、繁體中文、English (US)、English (UK)、日本語、한국어、Français、Deutsch、Español、Italiano、Русский、Português (BR)、Polski、Nederlands、Türkçe、Українська、Tiếng Việt、Bahasa Indonesia、ไทย、हिन्दी
- 语言切换会同时改变界面、托盘菜单、窗口标题与右键属性里的软件名称

### 其他

- **关闭到托盘** 或 **直接退出**，可在设置中切换
- **开机自启**（写入当前用户的启动项，无需管理员权限）
- **鼠标穿透**：遮罩不拦截任何点击
- **截图排除**：使用 `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`，各类截图软件截图的画面中不会获取到黑色遮罩。
- **单实例**：重复启动会唤起已有窗口
- **便携**：配置写入在 exe 同目录的 `config.json`。也不动注册表。

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
```

>产物：`target/release/night_viewing.exe`

图标与版本信息资源通过 `build.rs` 调用 `rc.exe` 编译 `assets/app.rc`；若环境里找不到资源编译器，会跳过该步（程序仍可编译运行，只是 exe 没有图标与版本信息）。可以用环境变量 `RC` 指定 `rc.exe` 的绝对路径。

## 实现要点

- **纯原生 Win32**：界面是 GDI 自绘（双缓冲 `CreateCompatibleDC` + `BitBlt`），没有引入任何 GUI 框架
- **分层窗口**：每块显示器一个 `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` 的弹出窗口，用 `SetLayeredWindowAttributes` 调 alpha
- **DPI**：进程启动即 `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`，并用 `GetDpiForWindow` 适配 100% ~ 200% 缩放
- **热键**：`RegisterHotKey` 挂在消息专用窗口上，注册失败自动降级到备用键位，并在日志中记录原因
- **内存**：常驻托盘时私有工作集约 450 KB，打开主界面约 420 KB（任务管理器「内存」列同口径），进程总工作集约 750 KB ~ 1.2 MB；无 GPU 渲染、无 WebView，release 构建开启 `lto`、`opt-level = "s"`、`panic = "abort"`、`strip`，并每 10 秒 `SetProcessWorkingSetSize` 回收工作集

## 已知限制

- 仅支持 Windows 10 / 11 x64（依赖分层窗口与 `WDA_EXCLUDEFROMCAPTURE`）
- 遮罩层无法覆盖 UAC 安全桌面、Ctrl+Alt+Del 界面与登录界面
