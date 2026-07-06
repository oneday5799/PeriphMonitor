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
  <img src="https://img.shields.io/badge/Platform-Windows%2010%2F11-0078d4?style=flat-square&logo=windows" alt="Platform">
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

### 构建

```bash
npm install
cargo tauri dev
```

### 许可证

[MIT](LICENSE)

### 致谢

- [BlueGauge](https://github.com/iKineticate/BlueGauge) — 蓝牙电量读取方案参考，windows_pnp 库来源
