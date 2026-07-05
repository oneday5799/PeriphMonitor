const CATEGORY_ORDER = [
  { key: "Audio", label: "音频设备" },
  { key: "Usb", label: "输入设备" },
  { key: "Bluetooth", label: "蓝牙设备" },
  { key: "Battery", label: "电池" },
  { key: "Monitor", label: "显示器" },
  { key: "Other", label: "其他设备" },
];

let allDevices = [];
let hiddenDevices = [];
let hiddenGroups = [];
let deviceNames = {};
let deviceGroups = {};

function getInvoke() {
  return window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : null;
}

function getListen() {
  return window.__TAURI__ && window.__TAURI__.event
    ? window.__TAURI__.event.listen
    : null;
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
    renderDevices();
  } catch (e) {
    list.innerHTML = `<div class="loading">加载失败: ${e}</div>`;
  }
}

function getDisplayName(dev) {
  return deviceNames[dev.name] || dev.name;
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
  for (const cat of CATEGORY_ORDER) {
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

      const nameEl = document.createElement("div");
      nameEl.className = "device-name";
      nameEl.textContent = getDisplayName(dev);
      card.appendChild(nameEl);

      const statusEl = document.createElement("div");
      statusEl.className = "device-status";
      statusEl.textContent = dev.battery != null ? `电量: ${dev.battery}%` : dev.status;
      card.appendChild(statusEl);

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
  input.value = getDisplayName(dev);
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

  for (const cat of CATEGORY_ORDER) {
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
  console.log("[popup] window focused, refreshing...");
  const invoke = getInvoke();
  if (!invoke) return;
  try {
    const cfg = await invoke("get_config");
    console.log("[popup] config loaded, hidden_groups:", cfg.hidden_groups);
    hiddenDevices = cfg.hidden_devices || [];
    hiddenGroups = cfg.hidden_groups || [];
    deviceNames = cfg.device_names || {};
    deviceGroups = cfg.device_groups || {};
    const devs = await invoke("get_devices");
    allDevices = devs;
    console.log("[popup] devices loaded:", devs.length, "rendering...");
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
