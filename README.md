# PeriphMonitor

Windows 系统托盘外设监控工具，实时显示所有连接设备的状态信息。

## 功能特性

- **实时设备检测** — 通过 WMI 查询和 WinRT 蓝牙 API，自动发现音频、USB、蓝牙、电池、显示器等设备
- **系统托盘** — 左键点击弹出设备列表，右键打开原生菜单
- **设备分组** — 按类型自动分类（音频、输入、蓝牙、电池、显示器、其他），支持自定义分组
- **分组可见性** — 在设置页中可独立控制每个分组的显示/隐藏
- **正则过滤** — 通过可编辑的正则表达式过滤设备，默认隐藏系统内置设备
- **设备管理** — 支持重命名设备、隐藏设备、更改设备分组
- **窗口状态记忆** — 设置页窗口大小和位置自动保存
- **开机自启动** — 支持通过设置页或右键菜单切换
- **TOML 配置** — 所有设置持久化为 TOML 格式，完整 UTF-8 支持

## 技术栈

| 层级 | 技术 |
|------|------|
| 框架 | Tauri v2 |
| 后端 | Rust |
| 前端 | 纯 HTML/CSS/JS（无框架） |
| 设备检测 | WMI + WinRT Bluetooth APIs |
| 异步运行时 | tokio（`spawn_blocking` 用于 WMI 查询） |
| 配置格式 | TOML |
| 插件 | tauri-plugin-autostart, tauri-plugin-single-instance, tauri-plugin-window-state |

## 项目结构

```
PeriphMonitor/
├── package.json
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/default.json
│   ├── icons/                 # 应用图标
│   ├── dist/                  # 前端文件
│   │   ├── popup.html         # 设备列表弹窗
│   │   ├── settings.html      # 设置页
│   │   ├── about.html         # 关于页
│   │   ├── scripts/
│   │   └── styles/
│   └── src/                   # Rust 后端
│       ├── main.rs            # 入口、托盘、窗口管理
│       ├── commands.rs        # IPC 命令
│       ├── config.rs          # TOML 配置
│       ├── device.rs          # 设备数据模型
│       └── wmi_query.rs       # WMI 设备查询
```

## 构建与运行

### 前置条件

- [Rust](https://rustup.rs/) (1.77.2+)
- [Node.js](https://nodejs.org/) (用于 npm)
- Windows 10/11

### 开发模式

```bash
npm install
cargo tauri dev
```

### 发布构建

```bash
cargo tauri build
```

## 配置文件

应用配置存储在 `config.toml` 中（与 exe 同目录）：

```toml
auto_start = false
hidden_devices = []
hidden_groups = ["Battery", "Monitor", "Other"]
filter_enabled = true
filter_regex = "..."  # 默认正则表达式
```

## 系统要求

- Windows 10 (1809+) 或 Windows 11
- 需要 WebView2 运行时（Windows 11 已内置，Windows 10 可能需要安装）

## 许可证

MIT License
