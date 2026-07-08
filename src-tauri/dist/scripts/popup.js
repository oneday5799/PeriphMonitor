let allDevices = [];
let hiddenDevices = [];
let hiddenGroups = [];
let deviceNames = {};
let deviceGroups = {};
let useSystemBt = false;

function showToast(msg) {
  let el = document.querySelector(".toast");
  if (!el) {
    el = document.createElement("div");
    el.className = "toast";
    document.body.appendChild(el);
  }
  el.textContent = msg;
  el.classList.add("show");
  clearTimeout(el._timer);
  el._timer = setTimeout(() => el.classList.remove("show"), 2000);
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

      const statusEl = document.createElement("div");
      statusEl.className = "device-status";
      if (dev.status === "已连接") {
        statusEl.classList.add("connected");
      } else if (dev.status === "已配对") {
        statusEl.classList.add("paired");
      }
      statusEl.textContent = dev.status;
      statusRow.appendChild(statusEl);

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
            showToast(isConnect ? "连接失败" : "断开失败");
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
  const overlay = document.createElement("div");
  overlay.className = "dialog-overlay";

  const dialog = document.createElement("div");
  dialog.className = "rename-dialog";

  const title = document.createElement("div");
  title.className = "dialog-title";
  title.textContent = "重命名设备";
  dialog.appendChild(title);

  const input = document.createElement("input");
  input.type = "text";
  input.className = "dialog-input";
  input.value = getDisplayName(dev, deviceNames);
  input.placeholder = "输入新名称";
  dialog.appendChild(input);

  const isRenamed = deviceNames[dev.name] !== undefined;

  const buttons = document.createElement("div");
  buttons.className = "dialog-buttons";

  if (isRenamed) {
    const restoreBtn = document.createElement("button");
    restoreBtn.className = "dialog-btn restore";
    restoreBtn.textContent = "恢复默认";
    restoreBtn.addEventListener("click", async () => {
      const invoke = getInvoke();
      if (invoke) {
        await invoke("rename_device", { original: dev.name, newName: "" });
        const config = await invoke("get_config");
        deviceNames = config.device_names || {};
        renderDevices();
      }
      overlay.remove();
    });
    buttons.appendChild(restoreBtn);
  }

  const cancelBtn = document.createElement("button");
  cancelBtn.className = "dialog-btn cancel";
  cancelBtn.textContent = "取消";
  cancelBtn.addEventListener("click", () => {
    overlay.remove();
  });

  const confirmBtn = document.createElement("button");
  confirmBtn.className = "dialog-btn confirm";
  confirmBtn.textContent = "确定";
  confirmBtn.addEventListener("click", async () => {
    const newName = input.value.trim();
    const invoke = getInvoke();
    if (invoke) {
      await invoke("rename_device", { original: dev.name, newName });
      const config = await invoke("get_config");
      deviceNames = config.device_names || {};
      renderDevices();
    }
    overlay.remove();
  });

  buttons.appendChild(cancelBtn);
  buttons.appendChild(confirmBtn);
  dialog.appendChild(buttons);

  overlay.appendChild(dialog);
  document.body.appendChild(overlay);

  input.focus();
  input.select();

  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") confirmBtn.click();
    if (e.key === "Escape") overlay.remove();
  });
}

function showGroupDialog(dev) {
  const overlay = document.createElement("div");
  overlay.className = "dialog-overlay";

  const dialog = document.createElement("div");
  dialog.className = "rename-dialog";

  const title = document.createElement("div");
  title.className = "dialog-title";
  title.textContent = "更改分组";
  dialog.appendChild(title);

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

  dialog.appendChild(groupList);

  const buttons = document.createElement("div");
  buttons.className = "dialog-buttons";

  // Restore default button (only if custom group)
  if (isCustomGroup) {
    const restoreBtn = document.createElement("button");
    restoreBtn.className = "dialog-btn restore";
    restoreBtn.textContent = "恢复默认";
    restoreBtn.addEventListener("click", async () => {
      const invoke = getInvoke();
      if (invoke) {
        await invoke("change_device_group", { name: dev.name, group: "" });
        const config = await invoke("get_config");
        deviceGroups = config.device_groups || {};
        renderDevices();
      }
      overlay.remove();
    });
    buttons.appendChild(restoreBtn);
  }

  const cancelBtn = document.createElement("button");
  cancelBtn.className = "dialog-btn cancel";
  cancelBtn.textContent = "取消";
  cancelBtn.addEventListener("click", () => {
    overlay.remove();
  });

  const confirmBtn = document.createElement("button");
  confirmBtn.className = "dialog-btn confirm";
  confirmBtn.textContent = "确定";
  confirmBtn.addEventListener("click", async () => {
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
    overlay.remove();
  });

  buttons.appendChild(cancelBtn);
  buttons.appendChild(confirmBtn);
  dialog.appendChild(buttons);

  overlay.appendChild(dialog);
  document.body.appendChild(overlay);
}

function hideContextMenu() {
  if (activeMenu) {
    activeMenu.remove();
    activeMenu = null;
  }
}

document.addEventListener("click", hideContextMenu);

document.getElementById("btn-refresh").addEventListener("click", loadDevices);

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
    const cfg = await invoke("get_config");
    hiddenDevices = cfg.hidden_devices || [];
    hiddenGroups = cfg.hidden_groups || [];
    deviceNames = cfg.device_names || {};
    deviceGroups = cfg.device_groups || {};
    useSystemBt = cfg.use_system_bt || false;
    allDevices = await invoke("get_devices");
    renderDevices();
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
