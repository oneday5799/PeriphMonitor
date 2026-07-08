// Shared constants and utilities for popup and settings pages

window.CATEGORIES = [
  { key: "Audio", label: "音频设备", subtitle: "扬声器、耳机等音频设备", icon: "🔊" },
  { key: "Usb", label: "输入设备", subtitle: "键盘、鼠标等USB设备", icon: "⌨️" },
  { key: "Wireless24G", label: "2.4G 无线设备", subtitle: "通过USB接收器连接的无线设备", icon: "📡" },
  { key: "Battery", label: "电池", subtitle: "电池设备", icon: "🔋" },
  { key: "Monitor", label: "显示器", subtitle: "显示器设备", icon: "🖥️" },
  { key: "Other", label: "其他设备", subtitle: "未归类的设备", icon: "📦" },
];

window.getInvoke = function () {
  return window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : null;
};

window.getListen = function () {
  return window.__TAURI__ && window.__TAURI__.event
    ? window.__TAURI__.event.listen
    : null;
};

window.groupDevices = function (devices, deviceGroups) {
  const groups = {};
  for (const d of devices) {
    const group = deviceGroups[d.name] || d.dt;
    if (!groups[group]) groups[group] = [];
    groups[group].push(d);
  }
  return groups;
};
