let allDevices = [];
let hiddenDevices = [];
let hiddenGroups = [];
let deviceNames = {};
let deviceGroups = {};
let useSystemBt = false;
let trayDevices = [];
let audioDevices = [];
let audioSessions = [];
let selectedDeviceId = null;

// Debounce helper
function debounce(fn, delay) {
  let timer = null;
  return function(...args) {
    clearTimeout(timer);
    timer = setTimeout(() => fn.apply(this, args), delay);
  };
}

function showToast(msg, onClick) {
  let el = document.querySelector(".toast");
  if (!el) {
    el = document.createElement("div");
    el.className = "toast";
    document.body.appendChild(el);
  }
  el.innerHTML = msg;
  el.classList.add("show");
  el.style.cursor = onClick ? "pointer" : "default";
  el.onclick = onClick || null;
  clearTimeout(el._timer);
  el._timer = setTimeout(() => {
    el.classList.remove("show");
    el.onclick = null;
    el.style.cursor = "default";
  }, 3000);
}

async function loadDevices() {
  const list = document.getElementById("device-list");
  list.innerHTML = '<div class="loading">加载中...</div>';

  const invoke = getInvoke();
  if (!invoke) {
    list.innerHTML = '<div class="loading">Tauri API 未加载</div>';
    return;
  }

  try {
    allDevices = await invoke("get_devices");
    const config = await invoke("get_config");
    hiddenDevices = config.hidden_devices || [];
    hiddenGroups = config.hidden_groups || [];
    deviceNames = config.device_names || {};
    deviceGroups = config.device_groups || {};
    useSystemBt = config.use_system_bt || false;
    trayDevices = config.tray_devices || [];
    renderDevices();
  } catch (e) {
    list.innerHTML = `<div class="loading">加载失败: ${e}</div>`;
  }
}

function getDeviceGroup(dev) {
  return deviceGroups[dev.name] || dev.dt;
}

function renderDevices() {
  const list = document.getElementById("device-list");
  list.innerHTML = "";

  const groups = {};
  for (const d of allDevices) {
    if (hiddenDevices.includes(d.name)) continue;
    const group = getDeviceGroup(d);
    if (!groups[group]) groups[group] = [];
    groups[group].push(d);
  }

  // Sort devices within each group: connected first, then paired, then non-BT/2.4G
  for (const group of Object.keys(groups)) {
    groups[group].sort((a, b) => {
      const getSortKey = (dev) => {
        // Bluetooth or 2.4G devices: connected first, paired middle
        if (dev.is_bluetooth || dev.is_wireless_24g) {
          return dev.status === "已连接" ? 0 : 1;
        }
        // Non-BT/2.4G devices: last
        return 2;
      };
      return getSortKey(a) - getSortKey(b);
    });
  }

  let hasContent = false;
  for (const cat of CATEGORIES) {
    if (hiddenGroups.includes(cat.key)) continue;
    const devs = groups[cat.key];
    if (!devs || devs.length === 0) continue;
    hasContent = true;

    const section = document.createElement("div");
    section.className = "category";

    const header = document.createElement("div");
    header.className = "category-header";
    header.textContent = cat.label;
    section.appendChild(header);

    for (const dev of devs) {
      const card = document.createElement("div");
      card.className = "device-card";

      // Device info container (name + status)
      const infoEl = document.createElement("div");
      infoEl.className = "device-info";

      const nameEl = document.createElement("div");
      nameEl.className = "device-name";
      nameEl.textContent = getDisplayName(dev, deviceNames);
      infoEl.appendChild(nameEl);

      const statusRow = document.createElement("div");
      statusRow.className = "device-status-row";

      // Only show connection status for Bluetooth or 2.4G devices
      if (dev.is_bluetooth || dev.is_wireless_24g) {
        const statusEl = document.createElement("div");
        statusEl.className = "device-status";
        if (dev.status === "已连接") {
          statusEl.classList.add("connected");
        } else if (dev.status === "已配对") {
          statusEl.classList.add("paired");
        }
        statusEl.textContent = dev.status;
        statusRow.appendChild(statusEl);
      }

      // Connection type label
      if (dev.is_bluetooth) {
        const tagEl = document.createElement("div");
        tagEl.className = "tag-bluetooth";
        tagEl.textContent = "蓝牙";
        statusRow.appendChild(tagEl);
      } else if (dev.is_wireless_24g) {
        const tagEl = document.createElement("div");
        tagEl.className = "tag-24g";
        tagEl.textContent = "2.4G";
        statusRow.appendChild(tagEl);
      }

      if (dev.battery != null) {
        const batteryEl = document.createElement("div");
        batteryEl.className = "device-battery";
        batteryEl.textContent = `${dev.battery}%`;
        statusRow.appendChild(batteryEl);
      }

      infoEl.appendChild(statusRow);
      card.appendChild(infoEl);

      // Connect/disconnect button for Bluetooth devices
      if (dev.is_bluetooth && (dev.status === "已配对" || dev.status === "已连接")) {
        const actionsEl = document.createElement("div");
        actionsEl.className = "device-actions";

        const connectBtn = document.createElement("button");
        connectBtn.className = "connect-btn";
        if (dev.status === "已连接") {
          connectBtn.textContent = "断开";
          connectBtn.dataset.action = "disconnect";
        } else {
          connectBtn.textContent = "连接";
          connectBtn.dataset.action = "connect";
        }
        connectBtn.addEventListener("click", async (e) => {
          e.stopPropagation();
          const invoke = getInvoke();
          if (!invoke) return;

          const isConnect = connectBtn.dataset.action === "connect";

          // If system BT mode is enabled, open Windows Bluetooth settings instead
          if (useSystemBt) {
            try {
              await invoke("open_bt_settings");
            } catch (err) {
              console.error("Failed to open BT settings:", err);
            }
            return;
          }

          // Disable button and show loading state
          connectBtn.disabled = true;
          connectBtn.classList.add("loading");

          // Update status text to show loading, hide battery
          const statusEl = card.querySelector(".device-status");
          const batteryEl = card.querySelector(".device-battery");
          if (statusEl) {
            statusEl.textContent = isConnect ? "正在连接..." : "正在断开...";
            statusEl.classList.remove("connected", "paired");
          }
          if (batteryEl) batteryEl.style.display = "none";

          const oldStatus = dev.status;

          try {
            if (isConnect) {
              await invoke("connect_bluetooth_device", { name: dev.name });
            } else {
              await invoke("disconnect_bluetooth_device", { name: dev.name });
            }
          } catch (err) {
            console.error("BT action failed:", err);
          }

          // Poll for status change with short interval
          const expectedConnected = isConnect;
          let newStatus = oldStatus;
          let statusChanged = false;
          // Connect needs initial delay for BT stack to stabilize; disconnect is instant
          const initialDelay = isConnect ? 800 : 100;
          await new Promise(r => setTimeout(r, initialDelay));
          const maxAttempts = 10; // 10 * 400ms = 4s max
          for (let i = 0; i < maxAttempts; i++) {
            try {
              const connected = await invoke("check_bt_connection", { name: dev.name });
              if (connected !== null && connected !== undefined) {
                newStatus = connected ? "已连接" : "已配对";
                if (connected === expectedConnected) {
                  statusChanged = true;
                  break;
                }
              }
            } catch (err) {
              console.error("Check connection failed:", err);
              break;
            }
            await new Promise(r => setTimeout(r, 400));
          }

          // Full refresh to get battery info and device list
          try {
            allDevices = await invoke("get_devices");
          } catch (err) {
            console.error("Refresh failed:", err);
          }

          // Use polling result if it detected change, otherwise use refresh result
          const refreshed = allDevices.find(d => d.name === dev.name);
          if (!statusChanged && refreshed) {
            newStatus = refreshed.status;
          }

          // Update only this card's status and button in-place
          const newStatusEl = card.querySelector(".device-status");
          const newBatteryEl = card.querySelector(".device-battery");
          if (newStatusEl) {
            newStatusEl.textContent = newStatus;
            newStatusEl.classList.remove("connected", "paired");
            if (newStatus === "已连接") newStatusEl.classList.add("connected");
            else if (newStatus === "已配对") newStatusEl.classList.add("paired");
          }
          if (newBatteryEl && refreshed && refreshed.battery != null) {
            newBatteryEl.textContent = `${refreshed.battery}%`;
            newBatteryEl.style.display = "";
          }
          connectBtn.disabled = false;
          connectBtn.classList.remove("loading");
          if (newStatus === "已连接") {
            connectBtn.textContent = "断开";
            connectBtn.dataset.action = "disconnect";
          } else if (newStatus === "已配对") {
            connectBtn.textContent = "连接";
            connectBtn.dataset.action = "connect";
          } else {
            connectBtn.style.display = "none";
          }

          if (!statusChanged) {
            const invoke = getInvoke();
            showToast(
              `${isConnect ? "连接失败" : "断开失败"}，点击这里跳转到系统设置进行修改`,
              invoke ? () => invoke("open_bt_settings") : null
            );
          }
        });
        actionsEl.appendChild(connectBtn);
        card.appendChild(actionsEl);
      }

      card.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        showContextMenu(e.clientX, e.clientY, dev);
      });

      section.appendChild(card);
    }

    list.appendChild(section);
  }

  if (!hasContent) {
    list.innerHTML = '<div class="loading">未检测到设备</div>';
  }
}

let activeMenu = null;

function showContextMenu(x, y, dev) {
  hideContextMenu();
  const invoke = getInvoke();
  if (!invoke) return;

  const menu = document.createElement("div");
  menu.className = "context-menu";

  // Rename option
  const renameItem = document.createElement("div");
  renameItem.className = "context-menu-item";
  renameItem.textContent = "重命名";
  renameItem.addEventListener("click", () => {
    hideContextMenu();
    showRenameDialog(dev);
  });
  menu.appendChild(renameItem);

  // Change group option
  const groupItem = document.createElement("div");
  groupItem.className = "context-menu-item";
  groupItem.textContent = "更改分组";
  groupItem.addEventListener("click", () => {
    hideContextMenu();
    showGroupDialog(dev);
  });
  menu.appendChild(groupItem);

  // Hide option
  const hideItem = document.createElement("div");
  hideItem.className = "context-menu-item";
  hideItem.textContent = "隐藏";
  hideItem.addEventListener("click", async () => {
    await invoke("toggle_device_hidden", { name: dev.name });
    const config = await invoke("get_config");
    hiddenDevices = config.hidden_devices || [];
    renderDevices();
    hideContextMenu();
  });
  menu.appendChild(hideItem);

  // Tray option
  const isTray = trayDevices.includes(dev.name);
  const trayItem = document.createElement("div");
  trayItem.className = "context-menu-item";
  trayItem.textContent = isTray ? "从托盘移除" : "添加到托盘";
  trayItem.addEventListener("click", async () => {
    try {
      await invoke("toggle_device_tray", { name: dev.name });
      if (trayDevices.includes(dev.name)) {
        trayDevices = trayDevices.filter(n => n !== dev.name);
      } else {
        trayDevices.push(dev.name);
      }
    } catch (e) {
      showToast(e);
    }
    hideContextMenu();
  });
  menu.appendChild(trayItem);

  document.body.appendChild(menu);

  // Smart boundary avoidance
  const menuW = menu.offsetWidth;
  const menuH = menu.offsetHeight;
  let posX = x;
  let posY = y;

  if (x + menuW > window.innerWidth) posX = x - menuW;
  if (y + menuH > window.innerHeight) posY = y - menuH;
  if (posX < 0) posX = 0;
  if (posY < 0) posY = 0;

  menu.style.left = posX + "px";
  menu.style.top = posY + "px";
  activeMenu = menu;
}

function showRenameDialog(dev) {
  const input = document.createElement("input");
  input.type = "text";
  input.className = "dialog-input";
  input.value = getDisplayName(dev, deviceNames);
  input.placeholder = "输入新名称";

  const isRenamed = deviceNames[dev.name] !== undefined;

  const buttons = [];

  if (isRenamed) {
    buttons.push({
      text: "恢复默认",
      className: "restore",
      onClick: async () => {
        const invoke = getInvoke();
        if (invoke) {
          await invoke("rename_device", { original: dev.name, newName: "" });
          const config = await invoke("get_config");
          deviceNames = config.device_names || {};
          renderDevices();
        }
        closeDialog(overlay);
      },
    });
  }

  buttons.push({
    text: "取消",
    className: "cancel",
    onClick: () => closeDialog(overlay),
  });

  buttons.push({
    text: "确定",
    className: "confirm",
    onClick: async () => {
      const newName = input.value.trim();
      const invoke = getInvoke();
      if (invoke) {
        await invoke("rename_device", { original: dev.name, newName });
        const config = await invoke("get_config");
        deviceNames = config.device_names || {};
        renderDevices();
      }
      closeDialog(overlay);
    },
  });

  const overlay = createDialog({
    title: "重命名设备",
    content: [input],
    buttons,
  });

  input.focus();
  input.select();

  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") overlay.querySelector(".dialog-btn.confirm")?.click();
  });
}

function showGroupDialog(dev) {
  const currentGroup = getDeviceGroup(dev);
  const isCustomGroup = deviceGroups[dev.name] !== undefined;

  const groupList = document.createElement("div");
  groupList.className = "group-list";

  for (const cat of CATEGORIES) {
    const item = document.createElement("div");
    item.className = "group-option" + (cat.key === currentGroup ? " selected" : "");
    item.textContent = cat.label;
    item.dataset.group = cat.key;

    item.addEventListener("click", () => {
      groupList.querySelectorAll(".group-option").forEach(el => el.classList.remove("selected"));
      item.classList.add("selected");
    });

    groupList.appendChild(item);
  }

  const buttons = [];

  if (isCustomGroup) {
    buttons.push({
      text: "恢复默认",
      className: "restore",
      onClick: async () => {
        const invoke = getInvoke();
        if (invoke) {
          await invoke("change_device_group", { name: dev.name, group: "" });
          const config = await invoke("get_config");
          deviceGroups = config.device_groups || {};
          renderDevices();
        }
        closeDialog(overlay);
      },
    });
  }

  buttons.push({
    text: "取消",
    className: "cancel",
    onClick: () => closeDialog(overlay),
  });

  buttons.push({
    text: "确定",
    className: "confirm",
    onClick: async () => {
      const selected = groupList.querySelector(".group-option.selected");
      if (selected) {
        const newGroup = selected.dataset.group;
        const invoke = getInvoke();
        if (invoke) {
          await invoke("change_device_group", {
            name: dev.name,
            group: newGroup === dev.dt ? "" : newGroup
          });
          const config = await invoke("get_config");
          deviceGroups = config.device_groups || {};
          renderDevices();
        }
      }
      closeDialog(overlay);
    },
  });

  const overlay = createDialog({
    title: "更改分组",
    content: [groupList],
    buttons,
  });
}

function hideContextMenu() {
  if (activeMenu) {
    activeMenu.remove();
    activeMenu = null;
  }
}

document.addEventListener("click", hideContextMenu);

// Refresh button - only refresh current tab
document.getElementById("btn-refresh").addEventListener("click", () => {
  const activeTab = document.querySelector('.tab-title.active');
  if (activeTab) {
    const tabName = activeTab.dataset.tab;
    if (tabName === 'devices') {
      loadDevices();
    } else if (tabName === 'volume') {
      loadAudioDevices();
      if (selectedDeviceId) {
        loadAudioSessions(selectedDeviceId);
      }
    }
  }
});

document.getElementById("btn-settings").addEventListener("click", async () => {
  const invoke = getInvoke();
  if (invoke) {
    try { await invoke("open_settings"); } catch (e) { console.error(e); }
  }
});

// Refresh config and devices when window regains focus
window.addEventListener("focus", async () => {
  const invoke = getInvoke();
  if (!invoke) return;
  try {
    // 保存当前滚动位置
    const volumeTab = document.getElementById('tab-volume');
    const deviceTab = document.getElementById('tab-devices');
    const scrollTop = (volumeTab.style.display !== 'none' ? volumeTab : deviceTab).scrollTop;

    const cfg = await invoke("get_config");
    hiddenDevices = cfg.hidden_devices || [];
    hiddenGroups = cfg.hidden_groups || [];
    deviceNames = cfg.device_names || {};
    deviceGroups = cfg.device_groups || {};
    useSystemBt = cfg.use_system_bt || false;
    trayDevices = cfg.tray_devices || [];
    allDevices = await invoke("get_devices");
    renderDevices();
    // 如果当前在音量页面，刷新应用音量列表
    if (volumeTab.style.display !== 'none') {
      await loadAudioDevices();
      if (selectedDeviceId) {
        await loadAudioSessions(selectedDeviceId);
      }
    }
    // 恢复滚动位置
    (volumeTab.style.display !== 'none' ? volumeTab : deviceTab).scrollTop = scrollTop;
  } catch (e) {
    console.error("Failed to refresh on focus:", e);
  }
});

if (window.__TAURI__) {
  loadDevices();
} else {
  window.addEventListener("DOMContentLoaded", () => {
    setTimeout(loadDevices, 100);
  });
}

// Tab switching
document.querySelectorAll('.tab-title').forEach(tab => {
  tab.addEventListener('click', async () => {
    document.querySelectorAll('.tab-title').forEach(t => t.classList.remove('active'));
    tab.classList.add('active');
    const tabName = tab.dataset.tab;
    document.getElementById('tab-devices').style.display = tabName === 'devices' ? 'block' : 'none';
    document.getElementById('tab-volume').style.display = tabName === 'volume' ? 'block' : 'none';
    if (tabName === 'volume') {
      await loadAudioDevices();
      // 刷新应用音量列表
      if (selectedDeviceId) {
        await loadAudioSessions(selectedDeviceId);
      }
    }
  });
});

function updateSessionCard(session) {
  const cards = document.querySelectorAll('.audio-session-card');
  for (const card of cards) {
    if (card.dataset.sessionId === session.id) {
      const slider = card.querySelector('.volume-slider');
      if (slider && document.activeElement !== slider) {
        slider.value = Math.round(session.volume * 100);
        updateSliderGradient(slider);
      }
      const valueEl = card.querySelector('.volume-value');
      if (valueEl) {
        valueEl.textContent = `${Math.round(session.volume * 100)}%`;
      }
      const muteBtn = card.querySelector('.mute-btn');
      if (muteBtn) {
        muteBtn.className = "mute-btn" + (session.is_muted ? " muted" : "");
        muteBtn.innerHTML = session.is_muted ? getMuteIcon() : getVolumeIcon();
      }
      break;
    }
  }
}

// Listen for volume change events from Rust backend
if (window.__TAURI__ && window.__TAURI__.event) {
  window.__TAURI__.event.listen('volume-changed', (event) => {
    const changes = event.payload;
    if (Array.isArray(changes)) {
      for (const change of changes) {
        // Check if it's a device change
        const device = audioDevices.find(d => d.id === change.device_id);
        if (device) {
          device.volume = change.volume;
          device.is_muted = change.is_muted;
          updateDeviceCard(device);
        }
        // Check if it's a session change
        if (change.session_id) {
          const session = audioSessions.find(s => s.id === change.session_id);
          if (session) {
            session.volume = change.volume;
            session.is_muted = change.is_muted;
            updateSessionCard(session);
          }
        }
      }
    }
  });
}

function updateDeviceCard(device) {
  const cards = document.querySelectorAll('.audio-device-card');
  let targetCard = null;
  for (const card of cards) {
    if (card.dataset.deviceId === device.id) {
      targetCard = card;
      break;
    }
  }
  if (!targetCard) return;

  const slider = targetCard.querySelector('.volume-slider');
  if (slider && document.activeElement !== slider) {
    slider.value = Math.round(device.volume * 100);
    updateSliderGradient(slider);
  }
  const valueEl = targetCard.querySelector('.volume-value');
  if (valueEl) {
    valueEl.textContent = `${Math.round(device.volume * 100)}%`;
  }
  const muteBtn = targetCard.querySelector('.mute-btn');
  if (muteBtn) {
    muteBtn.className = "mute-btn" + (device.is_muted ? " muted" : "");
    muteBtn.innerHTML = device.is_muted ? getMuteIcon() : getVolumeIcon();
  }
}

function updateSliderGradient(slider) {
  const value = slider.value;
  const percentage = ((value - slider.min) / (slider.max - slider.min)) * 100;
  // Update the track pseudo-element's background
  slider.style.setProperty('--track-color', `linear-gradient(to right, #0078d7 0%, #0078d7 ${percentage}%, #e0e0e0 ${percentage}%, #e0e0e0 100%)`);
}

async function loadAudioDevices() {
  const list = document.getElementById("audio-device-list");
  const invoke = getInvoke();
  if (!invoke) {
    return;
  }
  try {
    const devices = await invoke("get_audio_devices");
    console.log("[Volume] Loaded devices:", JSON.stringify(devices.map(d => ({id: d.id, name: d.name, volume: d.volume}))));
    audioDevices = devices;
    renderAudioDevices();
    if (audioDevices.length > 0 && !selectedDeviceId) {
      selectDevice(audioDevices[0].id);
    }
  } catch (e) {
    // 仅在没有现有内容时显示错误
    if (list.querySelectorAll('.audio-device-card').length === 0) {
      list.innerHTML = `<div class="loading">加载失败: ${e}</div>`;
    }
  }
}

function renderAudioDevices() {
  const list = document.getElementById("audio-device-list");
  if (audioDevices.length === 0) {
    list.innerHTML = '<div class="loading">没有检测到音频设备</div>';
    return;
  }

  // 清除加载指示器
  list.querySelectorAll('.loading').forEach(el => el.remove());

  // 获取现有卡片
  const existingCards = new Map();
  list.querySelectorAll('.audio-device-card').forEach(card => {
    existingCards.set(card.dataset.deviceId, card);
  });

  const newIds = new Set(audioDevices.map(d => d.id));

  // 移除不再存在的卡片
  existingCards.forEach((card, id) => {
    if (!newIds.has(id)) {
      card.remove();
    }
  });

  // 更新或添加卡片
  for (const device of audioDevices) {
    let card = existingCards.get(device.id);

    if (card) {
      // 更新现有卡片
      updateAudioDeviceCard(card, device);
    } else {
      // 创建新卡片
      card = createAudioDeviceCard(device);
      list.appendChild(card);
    }
  }
}

function createAudioDeviceCard(device) {
  const card = document.createElement("div");
  card.className = "audio-device-card";
  card.dataset.deviceId = device.id;
  if (device.id === selectedDeviceId) {
    card.classList.add("selected");
  }

  const header = document.createElement("div");
  header.className = "audio-device-header";

  const nameEl = document.createElement("div");
  nameEl.className = "audio-device-name" + (device.is_default ? " default" : "");
  nameEl.textContent = device.name;
  if (device.is_default) {
    const badge = document.createElement("span");
    badge.className = "default-badge";
    badge.textContent = "(默认)";
    nameEl.appendChild(badge);
  }
  header.appendChild(nameEl);
  card.appendChild(header);

  const controls = document.createElement("div");
  controls.className = "audio-device-controls";

  const slider = document.createElement("input");
  slider.type = "range";
  slider.className = "volume-slider";
  slider.min = "0";
  slider.max = "100";
  slider.value = Math.round(device.volume * 100);

  const debouncedSetDeviceVolume = debounce(async (id, vol) => {
    await setDeviceVolume(id, vol);
  }, 150);

  slider.addEventListener("input", (e) => {
    const value = parseInt(e.target.value) / 100;
    device.volume = value;
    updateVolumeDisplay(device.id, e.target.value);
    updateSliderGradient(e.target);
    debouncedSetDeviceVolume(device.id, value);
  });
  updateSliderGradient(slider);
  controls.appendChild(slider);

  const valueEl = document.createElement("span");
  valueEl.className = "volume-value";
  valueEl.id = `volume-value-${device.id}`;
  valueEl.textContent = `${Math.round(device.volume * 100)}%`;
  controls.appendChild(valueEl);

  const muteBtn = document.createElement("button");
  muteBtn.className = "mute-btn" + (device.is_muted ? " muted" : "");
  muteBtn.innerHTML = device.is_muted ? getMuteIcon() : getVolumeIcon();
  muteBtn.addEventListener("click", () => toggleDeviceMute(device.id));
  controls.appendChild(muteBtn);

  card.appendChild(controls);

  // Click to select device and show its sessions
  card.addEventListener("click", (e) => {
    if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'BUTTON') {
      selectDevice(device.id);
    }
  });

  return card;
}

function updateAudioDeviceCard(card, device) {
  // 更新选中状态
  if (device.id === selectedDeviceId) {
    card.classList.add("selected");
  } else {
    card.classList.remove("selected");
  }

  // 更新音量滑块（仅当用户未在拖动时）
  const slider = card.querySelector('.volume-slider');
  if (slider && document.activeElement !== slider) {
    slider.value = Math.round(device.volume * 100);
    updateSliderGradient(slider);
  }

  // 更新音量显示
  const valueEl = card.querySelector('.volume-value');
  if (valueEl) {
    valueEl.textContent = `${Math.round(device.volume * 100)}%`;
  }

  // 更新静音按钮
  const muteBtn = card.querySelector('.mute-btn');
  if (muteBtn) {
    muteBtn.className = "mute-btn" + (device.is_muted ? " muted" : "");
    muteBtn.innerHTML = device.is_muted ? getMuteIcon() : getVolumeIcon();
  }
}

function selectDevice(deviceId) {
  selectedDeviceId = deviceId;
  renderAudioDevices();
  loadAudioSessions(deviceId);
}

async function loadAudioSessions(deviceId) {
  const list = document.getElementById("audio-session-list");
  const invoke = getInvoke();
  if (!invoke) {
    return;
  }
  try {
    audioSessions = await invoke("get_audio_sessions", { deviceId });
    renderAudioSessions();
  } catch (e) {
    // 仅在没有现有内容时显示错误
    if (list.querySelectorAll('.audio-session-card').length === 0) {
      list.innerHTML = `<div class="loading">加载失败: ${e}</div>`;
    }
  }
}

function renderAudioSessions() {
  const list = document.getElementById("audio-session-list");
  if (audioSessions.length === 0) {
    list.innerHTML = '<div class="loading">没有正在播放的应用</div>';
    return;
  }

  // 清除加载指示器
  list.querySelectorAll('.loading').forEach(el => el.remove());

  // 获取现有卡片
  const existingCards = new Map();
  list.querySelectorAll('.audio-session-card').forEach(card => {
    existingCards.set(card.dataset.sessionId, card);
  });

  const newIds = new Set(audioSessions.map(s => s.id));

  // 移除不再存在的卡片
  existingCards.forEach((card, id) => {
    if (!newIds.has(id)) {
      card.remove();
    }
  });

  // 更新或添加卡片
  for (const session of audioSessions) {
    let card = existingCards.get(session.id);

    if (card) {
      // 更新现有卡片
      updateAudioSessionCard(card, session);
    } else {
      // 创建新卡片
      card = createAudioSessionCard(session);
      list.appendChild(card);
    }
  }
}

function createAudioSessionCard(session) {
  const card = document.createElement("div");
  card.className = "audio-session-card";
  card.dataset.sessionId = session.id;

  const iconEl = document.createElement("div");
  iconEl.className = "session-icon";
  if (session.icon && session.icon.length > 100) {
    const img = document.createElement("img");
    img.src = `data:image/png;base64,${session.icon}`;
    img.style.width = "100%";
    img.style.height = "100%";
    img.style.borderRadius = "4px";
    img.onerror = () => { iconEl.textContent = session.name.charAt(0).toUpperCase(); };
    iconEl.appendChild(img);
  } else {
    iconEl.textContent = session.name.charAt(0).toUpperCase();
    iconEl.style.background = stringToColor(session.name);
    iconEl.style.color = "#fff";
    iconEl.style.fontWeight = "bold";
  }
  card.appendChild(iconEl);

  const controls = document.createElement("div");
  controls.className = "session-controls";

  const slider = document.createElement("input");
  slider.type = "range";
  slider.className = "volume-slider session-slider";
  slider.min = "0";
  slider.max = "100";
  slider.value = Math.round(session.volume * 100);

  const debouncedSetSessionVolume = debounce(async (sessionId, vol) => {
    await setSessionVolume(sessionId, vol);
  }, 150);

  slider.addEventListener("input", async (e) => {
    const value = parseInt(e.target.value) / 100;
    // 更新本地状态中的音量
    const sess = audioSessions.find(s => s.id === card.dataset.sessionId);
    if (sess) sess.volume = value;
    updateSliderGradient(e.target);
    const valEl = card.querySelector('.volume-value');
    if (valEl) valEl.textContent = `${e.target.value}%`;
    debouncedSetSessionVolume(card.dataset.sessionId, value);
  });
  updateSliderGradient(slider);
  controls.appendChild(slider);

  const valueEl = document.createElement("span");
  valueEl.className = "volume-value";
  valueEl.textContent = `${Math.round(session.volume * 100)}%`;
  controls.appendChild(valueEl);

  const muteBtn = document.createElement("button");
  muteBtn.className = "mute-btn" + (session.is_muted ? " muted" : "");
  muteBtn.innerHTML = session.is_muted ? getMuteIcon() : getVolumeIcon();
  muteBtn.addEventListener("click", async () => {
    const sessionId = card.dataset.sessionId;
    await toggleSessionMute(sessionId);
    // 更新本地状态
    const sess = audioSessions.find(s => s.id === sessionId);
    if (sess) {
      sess.is_muted = !sess.is_muted;
      muteBtn.className = "mute-btn" + (sess.is_muted ? " muted" : "");
      muteBtn.innerHTML = sess.is_muted ? getMuteIcon() : getVolumeIcon();
    }
  });
  controls.appendChild(muteBtn);

  card.appendChild(controls);
  return card;
}

function updateAudioSessionCard(card, session) {
  // 更新音量滑块（仅当用户未在拖动时）
  const slider = card.querySelector('.volume-slider');
  if (slider && document.activeElement !== slider) {
    slider.value = Math.round(session.volume * 100);
    updateSliderGradient(slider);
  }

  // 更新音量显示
  const valueEl = card.querySelector('.volume-value');
  if (valueEl) {
    valueEl.textContent = `${Math.round(session.volume * 100)}%`;
  }

  // 更新静音按钮
  const muteBtn = card.querySelector('.mute-btn');
  if (muteBtn) {
    muteBtn.className = "mute-btn" + (session.is_muted ? " muted" : "");
    muteBtn.innerHTML = session.is_muted ? getMuteIcon() : getVolumeIcon();
  }
}

async function setDeviceVolume(deviceId, volume) {
  const invoke = getInvoke();
  if (!invoke) return;
  try {
    await invoke("set_device_volume", { deviceId, volume });
  } catch (e) {
    console.error("Failed to set volume:", e);
  }
}

async function toggleDeviceMute(deviceId) {
  const invoke = getInvoke();
  if (!invoke) return;
  try {
    await invoke("toggle_device_mute", { deviceId });
    const device = audioDevices.find(d => d.id === deviceId);
    if (device) {
      device.is_muted = !device.is_muted;
      renderAudioDevices();
    }
  } catch (e) {
    console.error("Failed to toggle mute:", e);
  }
}

async function setSessionVolume(sessionId, volume) {
  const invoke = getInvoke();
  if (!invoke) return;
  try {
    await invoke("set_session_volume", { sessionId, volume });
  } catch (e) {
    console.error("Failed to set session volume:", e);
  }
}

async function toggleSessionMute(sessionId) {
  const invoke = getInvoke();
  if (!invoke) return;
  try {
    await invoke("toggle_session_mute", { sessionId });
  } catch (e) {
    console.error("Failed to toggle session mute:", e);
  }
}

function updateVolumeDisplay(deviceId, value) {
  const valueEl = document.getElementById(`volume-value-${deviceId}`);
  if (valueEl) {
    valueEl.textContent = `${value}%`;
  }
}

function getVolumeIcon() {
  return `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
    <path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>
    <path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
  </svg>`;
}

function getMuteIcon() {
  return `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
    <line x1="23" y1="9" x2="17" y2="15"/>
    <line x1="17" y1="9" x2="23" y2="15"/>
  </svg>`;
}

function stringToColor(str) {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  const hue = Math.abs(hash) % 360;
  return `hsl(${hue}, 60%, 50%)`;
}
