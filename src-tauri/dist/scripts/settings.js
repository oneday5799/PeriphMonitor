const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let config = null;
let devices = [];
let expandedGroups = new Set();
let deviceGroups = {};

async function init() {
  // Tab switching
  document.querySelectorAll(".tab-item").forEach(tab => {
    tab.addEventListener("click", () => {
      document.querySelectorAll(".tab-item").forEach(t => t.classList.remove("active"));
      document.querySelectorAll(".tab-content").forEach(c => c.classList.remove("active"));
      tab.classList.add("active");
      document.getElementById("tab-" + tab.dataset.tab).classList.add("active");
    });
  });

  try {
    config = await invoke("get_config");

    // Auto-start toggle
    const toggle = document.getElementById("toggle-autostart");
    toggle.checked = config.auto_start;
    toggle.addEventListener("change", async () => {
      config.auto_start = toggle.checked;
      await invoke("update_config", { newConfig: config });
    });

    // Filter toggle
    const filterToggle = document.getElementById("toggle-filter");
    const filterWrap = document.getElementById("filter-regex-wrap");
    filterToggle.checked = config.filter_enabled;
    filterWrap.style.display = config.filter_enabled ? "block" : "none";
    filterToggle.addEventListener("change", async () => {
      config.filter_enabled = filterToggle.checked;
      filterWrap.style.display = filterToggle.checked ? "block" : "none";
      await invoke("update_config", { newConfig: config });
      await loadDevicesAsync();
    });

    // Filter regex input
    const regexInput = document.getElementById("filter-regex");
    regexInput.value = config.filter_regex || "";
    let debounceTimer = null;
    regexInput.addEventListener("input", () => {
      clearTimeout(debounceTimer);
      debounceTimer = setTimeout(async () => {
        config.filter_regex = regexInput.value;
        await invoke("update_config", { newConfig: config });
        loadDevicesAsync();
      }, 500);
    });

    // Dedup toggle
    const dedupToggle = document.getElementById("toggle-dedup");
    dedupToggle.checked = config.dedup_devices;
    dedupToggle.addEventListener("change", async () => {
      config.dedup_devices = dedupToggle.checked;
      await invoke("update_config", { newConfig: config });
      await loadDevicesAsync();
    });

    // Show unnamed BT devices toggle
    const unnamedBtToggle = document.getElementById("toggle-unnamed-bt");
    unnamedBtToggle.checked = config.show_unnamed_bt;
    unnamedBtToggle.addEventListener("change", async () => {
      config.show_unnamed_bt = unnamedBtToggle.checked;
      await invoke("update_config", { newConfig: config });
      await loadDevicesAsync();
    });

    // Use system Bluetooth connection toggle
    const useSystemBtToggle = document.getElementById("toggle-use-system-bt");
    useSystemBtToggle.checked = config.use_system_bt;
    useSystemBtToggle.addEventListener("change", async () => {
      config.use_system_bt = useSystemBtToggle.checked;
      await invoke("update_config", { newConfig: config });
    });

    // Logging settings
    const logToggle = document.getElementById("toggle-log");
    logToggle.checked = config.log_enabled;
    logToggle.addEventListener("change", async () => {
      config.log_enabled = logToggle.checked;
      await invoke("update_config", { newConfig: config });
    });

    const logLevelSelect = document.getElementById("log-level");
    logLevelSelect.value = config.log_level || "standard";
    logLevelSelect.addEventListener("change", async () => {
      config.log_level = logLevelSelect.value;
      await invoke("update_config", { newConfig: config });
    });

    const logRetentionSelect = document.getElementById("log-retention");
    logRetentionSelect.value = config.log_retention || "one_day";
    logRetentionSelect.addEventListener("change", async () => {
      config.log_retention = logRetentionSelect.value;
      await invoke("update_config", { newConfig: config });
    });

    document.getElementById("btn-log-dir").addEventListener("click", async () => {
      try {
        await invoke("open_log_dir");
      } catch (e) {
        console.error("Failed to open log dir:", e);
      }
    });

    loadDevicesAsync();
    loadAudioDevicesAsync();

    // Open 2.4G device list button
    document.getElementById("btn-add-24g").addEventListener("click", async () => {
      try {
        await invoke("open_24g_device_file");
      } catch (e) {
        console.error("Failed to open file:", e);
      }
    });
  } catch (e) {
    console.error("Failed to load settings:", e);
  }
}

async function loadDevicesAsync() {
  try {
    config = await invoke("get_config");
    devices = await invoke("get_devices");
    deviceGroups = config.device_groups || {};
    renderGroups();
  } catch (e) {
    console.error("Failed to load devices:", e);
  }
}

function renderGroups() {
  const container = document.getElementById("device-groups");
  container.innerHTML = "";

  const groups = {};
  for (const d of devices) {
    const group = deviceGroups[d.name] || d.dt;
    if (!groups[group]) groups[group] = [];
    groups[group].push(d);
  }

  for (const group of CATEGORIES) {
    const devs = groups[group.key] || [];
    const groupEl = document.createElement("div");
    groupEl.className = "group";

    const card = document.createElement("div");
    card.className = "group-card";

    const header = document.createElement("div");
    header.className = "group-header";

    const icon = document.createElement("div");
    icon.className = "group-icon";
    icon.textContent = group.icon;
    header.appendChild(icon);

    const textWrap = document.createElement("div");
    textWrap.className = "group-text";

    const title = document.createElement("div");
    title.className = "group-title";
    title.textContent = group.label;
    textWrap.appendChild(title);

    const subtitle = document.createElement("div");
    subtitle.className = "group-subtitle";
    subtitle.textContent = group.subtitle;
    textWrap.appendChild(subtitle);

    header.appendChild(textWrap);

    // Group toggle switch
    const groupToggle = document.createElement("label");
    groupToggle.className = "toggle group-toggle";
    const isGroupHidden = config.hidden_groups.includes(group.key);
    const groupInput = document.createElement("input");
    groupInput.type = "checkbox";
    groupInput.checked = !isGroupHidden;

    groupToggle.addEventListener("click", (e) => {
      e.stopPropagation();
    });
    groupInput.addEventListener("change", async (e) => {
      e.stopPropagation();
      await invoke("toggle_group_hidden", { group: group.key });
      const cfg = await invoke("get_config");
      config.hidden_groups = cfg.hidden_groups || [];
      renderGroups();
    });

    const groupSlider = document.createElement("span");
    groupSlider.className = "slider";
    groupToggle.appendChild(groupInput);
    groupToggle.appendChild(groupSlider);
    header.appendChild(groupToggle);

    // Chevron arrow
    const arrow = document.createElement("div");
    arrow.className = "group-arrow";
    arrow.innerHTML = `<svg width="12" height="12" viewBox="0 0 12 12" fill="none"><path d="M4 2L8 6L4 10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
    header.appendChild(arrow);

    const items = document.createElement("div");
    items.className = "group-items";

    header.addEventListener("click", () => {
      const isExpanded = items.classList.toggle("show");
      arrow.classList.toggle("expanded", isExpanded);
      if (isExpanded) {
        expandedGroups.add(group.key);
        items.style.maxHeight = items.scrollHeight + "px";
      } else {
        expandedGroups.delete(group.key);
        items.style.maxHeight = "0px";
      }
    });

    if (expandedGroups.has(group.key)) {
      items.classList.add("show");
      arrow.classList.add("expanded");
    }

    for (const dev of devs) {
      const item = document.createElement("div");
      item.className = "device-item";

      const nameEl = document.createElement("div");
      nameEl.className = "device-item-name";
      nameEl.textContent = dev.name;

      const isHidden = config.hidden_devices.includes(dev.name);
      if (isHidden) nameEl.classList.add("hidden");

      const toggle = document.createElement("label");
      toggle.className = "toggle";

      const input = document.createElement("input");
      input.type = "checkbox";
      input.checked = !isHidden;
      input.addEventListener("change", async () => {
        await invoke("toggle_device_hidden", { name: dev.name });
        config = await invoke("get_config");
        nameEl.classList.toggle("hidden", !input.checked);
      });

      const slider = document.createElement("span");
      slider.className = "slider";

      toggle.appendChild(input);
      toggle.appendChild(slider);

      item.appendChild(nameEl);
      item.appendChild(toggle);
      items.appendChild(item);
    }

    card.appendChild(header);
    card.appendChild(items);
    groupEl.appendChild(card);
    container.appendChild(groupEl);

    if (expandedGroups.has(group.key)) {
      requestAnimationFrame(() => {
        items.style.maxHeight = items.scrollHeight + "px";
      });
    }
  }
}

listen("config-changed", async () => {
  config = await invoke("get_config");
  loadDevicesAsync();
  loadAudioDevicesAsync();
});

async function loadAudioDevicesAsync() {
  try {
    config = await invoke("get_config");
    const audioDevices = await invoke("get_audio_devices");
    renderAudioDeviceGroups(audioDevices);
  } catch (e) {
    console.error("Failed to load audio devices:", e);
  }
}

function renderAudioDeviceGroups(audioDevices) {
  const container = document.getElementById("audio-device-groups");
  container.innerHTML = "";

  if (audioDevices.length === 0) {
    container.innerHTML = '<div class="device-item"><div class="device-item-name" style="color:#888">没有检测到音频设备</div></div>';
    return;
  }

  const groupEl = document.createElement("div");
  groupEl.className = "group";

  const card = document.createElement("div");
  card.className = "group-card";

  const items = document.createElement("div");
  items.className = "group-items show";
  items.style.maxHeight = "none";

  for (const dev of audioDevices) {
    const item = document.createElement("div");
    item.className = "device-item";

    const nameEl = document.createElement("div");
    nameEl.className = "device-item-name";
    nameEl.textContent = dev.name;
    if (dev.is_default) {
      const badge = document.createElement("span");
      badge.style.cssText = "font-size:12px;color:#0078d7;margin-left:6px";
      badge.textContent = "(默认)";
      nameEl.appendChild(badge);
    }

    const isHidden = (config.hidden_audio_devices || []).includes(dev.name);
    if (isHidden) nameEl.classList.add("hidden");

    const toggle = document.createElement("label");
    toggle.className = "toggle";

    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = !isHidden;
    input.addEventListener("change", async () => {
      await invoke("toggle_audio_device_hidden", { name: dev.name });
      config = await invoke("get_config");
      nameEl.classList.toggle("hidden", !input.checked);
    });

    const slider = document.createElement("span");
    slider.className = "slider";

    toggle.appendChild(input);
    toggle.appendChild(slider);

    item.appendChild(nameEl);
    item.appendChild(toggle);
    items.appendChild(item);
  }

  card.appendChild(items);
  groupEl.appendChild(card);
  container.appendChild(groupEl);
}

init();
