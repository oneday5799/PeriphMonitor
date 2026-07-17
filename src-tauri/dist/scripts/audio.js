let audioDevices = [];
let audioSessions = [];
let selectedDeviceId = null;

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

if (window.__TAURI__ && window.__TAURI__.event) {
  window.__TAURI__.event.listen('volume-changed', (event) => {
    const changes = event.payload;
    if (Array.isArray(changes)) {
      for (const change of changes) {
        const device = audioDevices.find(d => d.id === change.device_id);
        if (device) {
          device.volume = change.volume;
          device.is_muted = change.is_muted;
          updateDeviceCard(device);
        }
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

  list.querySelectorAll('.loading').forEach(el => el.remove());

  const existingCards = new Map();
  list.querySelectorAll('.audio-device-card').forEach(card => {
    existingCards.set(card.dataset.deviceId, card);
  });

  const newIds = new Set(audioDevices.map(d => d.id));

  existingCards.forEach((card, id) => {
    if (!newIds.has(id)) {
      card.remove();
    }
  });

  for (const device of audioDevices) {
    let card = existingCards.get(device.id);

    if (card) {
      updateAudioDeviceCard(card, device);
    } else {
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
  nameEl.addEventListener("click", async (e) => {
    e.stopPropagation();
    if (nameEl.classList.contains("default")) return;
    const invoke = getInvoke();
    if (!invoke) return;
    try {
      nameEl.classList.add("default");
      if (!nameEl.querySelector('.default-badge')) {
        const badge = document.createElement("span");
        badge.className = "default-badge";
        badge.textContent = "(默认)";
        nameEl.appendChild(badge);
      }
      await invoke("set_default_device", { deviceId: device.id });
      await new Promise(r => setTimeout(r, 500));
      await loadAudioDevices();
      selectDevice(device.id);
    } catch (err) {
      nameEl.classList.remove("default");
      const badge = nameEl.querySelector('.default-badge');
      if (badge) badge.remove();
      console.error("Failed to set default device:", err);
    }
  });
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

  const throttledSetDeviceVolume = throttle(async (id, vol) => {
    await setDeviceVolume(id, vol);
  }, 150);

  slider.addEventListener("input", (e) => {
    const value = parseInt(e.target.value) / 100;
    device.volume = value;
    updateVolumeDisplay(device.id, e.target.value);
    updateSliderGradient(e.target);
    throttledSetDeviceVolume(device.id, value);
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

  card.addEventListener("click", (e) => {
    if (e.target.tagName !== 'INPUT' && e.target.tagName !== 'BUTTON') {
      selectDevice(device.id);
    }
  });

  return card;
}

function updateAudioDeviceCard(card, device) {
  if (device.id === selectedDeviceId) {
    card.classList.add("selected");
  } else {
    card.classList.remove("selected");
  }

  const nameEl = card.querySelector('.audio-device-name');
  if (nameEl) {
    if (device.is_default) {
      nameEl.classList.add("default");
      if (!nameEl.querySelector('.default-badge')) {
        const badge = document.createElement("span");
        badge.className = "default-badge";
        badge.textContent = "(默认)";
        nameEl.appendChild(badge);
      }
    } else {
      nameEl.classList.remove("default");
      const badge = nameEl.querySelector('.default-badge');
      if (badge) badge.remove();
    }
  }

  const slider = card.querySelector('.volume-slider');
  if (slider && document.activeElement !== slider) {
    slider.value = Math.round(device.volume * 100);
    updateSliderGradient(slider);
  }

  const valueEl = card.querySelector('.volume-value');
  if (valueEl) {
    valueEl.textContent = `${Math.round(device.volume * 100)}%`;
  }

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

  list.querySelectorAll('.loading').forEach(el => el.remove());

  const existingCards = new Map();
  list.querySelectorAll('.audio-session-card').forEach(card => {
    existingCards.set(card.dataset.sessionId, card);
  });

  const newIds = new Set(audioSessions.map(s => s.id));

  existingCards.forEach((card, id) => {
    if (!newIds.has(id)) {
      card.remove();
    }
  });

  for (const session of audioSessions) {
    let card = existingCards.get(session.id);

    if (card) {
      updateAudioSessionCard(card, session);
    } else {
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

  const throttledSetSessionVolume = throttle(async (sessionId, vol) => {
    await setSessionVolume(sessionId, vol);
  }, 100);

  slider.addEventListener("input", async (e) => {
    const value = parseInt(e.target.value) / 100;
    const sess = audioSessions.find(s => s.id === card.dataset.sessionId);
    if (sess) sess.volume = value;
    updateSliderGradient(e.target);
    const valEl = card.querySelector('.volume-value');
    if (valEl) valEl.textContent = `${e.target.value}%`;
    throttledSetSessionVolume(card.dataset.sessionId, value);
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
