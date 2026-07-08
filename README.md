<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" alt="PeriphMonitor Logo">
</p>

<h1 align="center">PeriphMonitor</h1>

<p align="center">
  PeriphMonitor — 一款轻量级的 Windows 系统托盘外设监控工具，实时显示所有连接设备的状态信息。
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.77+-black?style=flat-square&logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/Tauri-2.x-blue?style=flat-square&logo=tauri" alt="Tauri">
  <img src="https://img.shields.io/badge/Platform-Windows%2010%2F11%20(x64%2FARM64)-0078d4?style=flat-square&logo=windows" alt="Platform">
  <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License">
</p>

---

## 简介

PeriphMonitor 是一款运行在 Windows 系统托盘中的轻量级外设监控工具。它通过 WMI 查询、WinRT 蓝牙 API 和 windows_pnp 库实时检测所有连接的外设设备，并以分类列表的形式展示设备状态。

### 主要功能

- 实时检测音频、USB、蓝牙、电池、显示器等设备
- 蓝牙设备显示连接/配对状态和电量百分比（BLE 设备通过 GATT Battery Service 读取）
- 系统托盘图标，左键弹出设备列表，右键原生菜单
- 设备分组管理，支持自定义分组和分组可见性控制
- 正则表达式过滤，可编辑过滤规则
- 设备去重开关，支持按蓝牙设备名后缀（Hands-Free、A2DP 等）去重
- 设备重命名与隐藏
- 显示无名称蓝牙设备开关（默认关闭）
- 使用系统蓝牙连接开关，支持跳转到 Windows 蓝牙设置页管理连接
- 设置页窗口状态自动记忆
- 开机自启动支持
- 单实例模式，重复启动时自动聚焦已有窗口

### 技术栈

| 组件 | 技术 |
|------|------|
| 框架 | Tauri v2 |
| 后端 | Rust |
| 前端 | 纯 HTML/CSS/JS |
| 设备检测 | WMI + WinRT Bluetooth + windows_pnp |
| BLE 电量 | GATT Battery Service (0x180F/0x2A19) |
| BTC 电量 | windows_pnp (DEVPKEY_BLUETOOTH_BATTERY) |
| 异步 | tokio |
| 配置 | TOML |

### 项目结构

```
src-tauri/src/
├── main.rs          # 应用入口，COM 初始化，Tauri 构建
├── state.rs         # 全局状态（托盘位置、动画状态、自启动）
├── classify.rs      # 设备分类逻辑（PNPClass/名称匹配）
├── bluetooth.rs     # WinRT 蓝牙 API（配对设备、GATT/PnP 电量）
├── wmi_query.rs     # WMI 查询编排，设备去重与过滤
├── commands.rs      # Tauri 命令处理器
├── config.rs        # 配置管理（TOML 加载/保存）
├── device.rs        # 数据模型（Device, DevType）
├── popup.rs         # 弹出窗口生命周期与动画
├── tray.rs          # 系统托盘菜单与事件
└── windows.rs       # 窗口创建与 DWM 圆角
```

### 设备过滤机制

项目使用多层过滤确保只显示有意义的设备：

1. **PNPClass 白名单**：仅查询 AudioEndpoint、Bluetooth、HIDClass、Keyboard、MEDIA、Mouse、Monitor
2. **PNPDeviceID 结构过滤**：基于设备 ID 格式过滤蓝牙服务、通用 HID 接口、系统组件
3. **正则表达式过滤**：可配置的设备名称过滤规则
4. **设备去重**：按核心名称去重，支持蓝牙后缀合并

### 构建

```bash
npm install
npm run tauri dev
```

### 下载

从 [Releases](https://github.com/oneday5799/PeriphMonitor/releases) 页面下载最新版本，支持 x64 和 ARM64 架构。

### CI/CD

推送 `v*` 格式的 tag 时自动触发 GitHub Actions 构建：

```bash
git tag v1.0.0-beta.1
git push origin v1.0.0-beta.1
```

工作流会自动：
- 从 tag 提取版本号并更新配置文件
- 并行构建 x64 和 ARM64 安装包
- 创建 GitHub Release（tag 名含 `-` 时标记为 Pre-release）

### 许可证

[MIT](LICENSE)

### 致谢

- [BlueGauge](https://github.com/iKineticate/BlueGauge) — 蓝牙电量读取方案参考，windows_pnp 库来源
- [BluetoothAutoConnect](https://github.com/lvusyy/BluetoothAutoConnect) — 蓝牙连接/断开 PowerShell 脚本参考
