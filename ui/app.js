// =========================================================
// Media Liquid Glass - Application Logic (Refined & Optimized)
// =========================================================

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// DOM Elements
const liquidCard = document.getElementById("liquidCard");
const albumArt = document.getElementById("albumArt");
const artPlaceholder = document.getElementById("artPlaceholder");
const ambientGlowA = document.getElementById("ambientGlowA");
const ambientGlowB = document.getElementById("ambientGlowB");
const titleBox = document.getElementById("titleBox");
const trackTitle = document.getElementById("trackTitle");
const trackArtist = document.getElementById("trackArtist");
const trackAlbum = document.getElementById("trackAlbum");
const appBadge = document.getElementById("appBadge");
const badgeName = document.getElementById("badgeName");
const timeCurrent = document.getElementById("timeCurrent");
const timeTotal = document.getElementById("timeTotal");
const scrubber = document.getElementById("scrubber");
const scrubberFill = document.getElementById("scrubberFill");
const scrubberHead = document.getElementById("scrubberHead");
const btnPlayPause = document.getElementById("btnPlayPause");
const iconPlay = document.getElementById("iconPlay");
const iconPause = document.getElementById("iconPause");
const btnPrev = document.getElementById("btnPrev");
const btnNext = document.getElementById("btnNext");
const btnSettings = document.getElementById("btnSettings");
const btnHide = document.getElementById("btnHide");
const settingsPanel = document.getElementById("settingsPanel");
const btnCloseSettings = document.getElementById("btnCloseSettings");
const btnSaveConfig = document.getElementById("btnSaveConfig");
const volumeToast = document.getElementById("volumeToast");

// Audio Source Popover Elements
const sourcePopover = document.getElementById("sourcePopover");
const sourceList = document.getElementById("sourceList");
const btnCloseSourcePopover = document.getElementById("btnCloseSourcePopover");

// Volume Controls Elements
const btnMute = document.getElementById("btnMute");
const iconVolHigh = document.getElementById("iconVolHigh");
const iconVolMute = document.getElementById("iconVolMute");
const volBar = document.getElementById("volBar");
const volFill = document.getElementById("volFill");
const volHead = document.getElementById("volHead");
const volLabel = document.getElementById("volLabel");

// Settings Elements
const modBtns = document.querySelectorAll(".mod-btn");
const keySelect = document.getElementById("keySelect");
const bindBadge = document.getElementById("bindBadge");
const posOptions = document.querySelectorAll(".pos-option");
const chkAutoHide = document.getElementById("chkAutoHide");
const chkAmbientGlow = document.getElementById("chkAmbientGlow");
const chkHoverReveal = document.getElementById("chkHoverReveal");
const chkAppVolume = document.getElementById("chkAppVolume");
const chkDraggable = document.getElementById("chkDraggable");
const chkRespectFullscreen = document.getElementById("chkRespectFullscreen");

// State
let isDraggingScrubber = false;
let currentDuration = 0;
let currentPosition = 0;
let currentIsPlaying = false;
let isSettingsOpen = false;
let settingsBusy = false;
let configDirty = false;
let currentWindowHeight = 188;
let isSourcePopoverOpen = false;
let selectedSourceId = null;
let currentMasterVolume = 0.5;
let isMasterMuted = false;
let isDraggingVolume = false;
let appConfig = null;
let lastThumbnailBase64 = null;
let lastTrackKey = "";
let thumbFetchKey = "";
let glowFront = null; // layer currently visible (A/B), the other cross-fades in

// Cross-fade the ambient glow: set the new art on the hidden layer, then fade layers
function setAmbientGlow(src) {
  if (glowFront === null) glowFront = ambientGlowA;
  const back = glowFront === ambientGlowA ? ambientGlowB : ambientGlowA;
  if (src) {
    back.style.backgroundImage = `url(${src})`;
    back.style.opacity = "0.55";
    glowFront.style.opacity = "0";
    glowFront = back;
  } else {
    ambientGlowA.style.opacity = "0";
    ambientGlowB.style.opacity = "0";
  }
}

function disableAmbientGlow() {
  ambientGlowA.style.opacity = "0";
  ambientGlowB.style.opacity = "0";
}

// Derive a vivid accent color from the album art and apply it app-wide.
// ponytail: 24x24 canvas + saturation-weighted average; a real palette extractor (MMCQ) if colors look muddy
function applyAccentFromArt(src) {
  const img = new Image();
  img.onload = () => {
    try {
      const size = 24;
      const canvas = document.createElement("canvas");
      canvas.width = size;
      canvas.height = size;
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      ctx.drawImage(img, 0, 0, size, size);
      const data = ctx.getImageData(0, 0, size, size).data;
      let r = 0, g = 0, b = 0, wsum = 0;
      for (let i = 0; i < data.length; i += 4) {
        if (data[i + 3] < 128) continue;
        const max = Math.max(data[i], data[i + 1], data[i + 2]);
        const min = Math.min(data[i], data[i + 1], data[i + 2]);
        const sat = (max - min) / 255;
        const w = 0.15 + sat; // favor colorful pixels, skip near-grays
        r += data[i] * w;
        g += data[i + 1] * w;
        b += data[i + 2] * w;
        wsum += w;
      }
      if (!wsum) return;
      let cr = r / wsum, cg = g / wsum, cb = b / wsum;
      // Push saturation up and brightness into a usable range for text/glows
      const mx = Math.max(cr, cg, cb);
      if (mx > 0) {
        const boost = 1.35;
        cr = Math.min(255, cr * boost);
        cg = Math.min(255, cg * boost);
        cb = Math.min(255, cb * boost);
      }
      const lum = 0.2126 * cr + 0.7152 * cg + 0.0722 * cb;
      if (lum < 90) {
        const f = 120 / Math.max(1, lum);
        cr = Math.min(255, cr * f);
        cg = Math.min(255, cg * f);
        cb = Math.min(255, cb * f);
      }
      const root = document.documentElement.style;
      root.setProperty("--accent-rgb", `${Math.round(cr)}, ${Math.round(cg)}, ${Math.round(cb)}`);
      root.setProperty("--accent-dark", `rgb(${Math.round(cr * 0.75)}, ${Math.round(cg * 0.75)}, ${Math.round(cb * 0.75)})`);
    } catch (err) {
      console.warn("Accent extraction failed:", err);
    }
  };
  img.onerror = () => {};
  img.src = src;
}

function resetAccent() {
  const root = document.documentElement.style;
  root.removeProperty("--accent-rgb");
  root.removeProperty("--accent-dark");
}

function trackKeyOf(state) {
  return `${state.source_app_id || ""}::${state.title || ""}::${state.artist || ""}`;
}

// Centralized window resize: skips redundant OS resizes (each one re-layouts + re-blurs the card)
async function setWindowSize(width, height) {
  if (height === currentWindowHeight) return;
  currentWindowHeight = height;
  try {
    await invoke("set_window_size", { width, height });
  } catch (err) {
    console.error("set_window_size error:", err);
  }
}
let isOptimisticPlaying = null;
let volumeToastTimeout = null;

// Helper: Format seconds to M:SS
function formatTime(secs) {
  if (isNaN(secs) || secs < 0) return "0:00";
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s < 10 ? "0" : ""}${s}`;
}

// Clean and friendly App Name
function escapeHtml(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({"&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;"}[c]));
}

function formatSourceApp(appId) {
  if (!appId) return "Media";
  const lower = appId.toLowerCase();
  if (lower.includes("spotify")) return "Spotify";
  if (lower.includes("chrome")) return "Google Chrome";
  if (lower.includes("msedge") || lower.includes("edge")) return "Microsoft Edge";
  if (lower.includes("firefox")) return "Mozilla Firefox";
  if (lower.includes("brave")) return "Brave";
  if (lower.includes("vlc")) return "VLC Media Player";
  if (lower.includes("apple") || lower.includes("itunes")) return "Apple Music";
  if (lower.includes("tidal")) return "TIDAL";
  if (lower.includes("foobar")) return "foobar2000";
  return "Windows Media";
}

// Check and apply marquee scroll animation if text overflows container
function checkMarquee(element, container) {
  if (!element || !container) return;
  const overflow = element.scrollWidth - container.clientWidth;
  if (overflow > 4) {
    element.style.setProperty("--marquee-offset", `-${overflow + 12}px`);
    element.classList.add("scrollable");
  } else {
    element.classList.remove("scrollable");
  }
}

// Update UI with incoming MediaState
function updateMediaUI(state) {
  if (!state) return;

  // Title, Artist, Album
  const newTitle = state.title || "Sin reproducción";
  if (trackTitle.textContent !== newTitle) {
    trackTitle.textContent = newTitle;
    requestAnimationFrame(() => checkMarquee(trackTitle, titleBox));
  }

  // Only touch the DOM when the text actually changes (avoids style recalc every poll)
  const newArtist = state.artist || "Inicia tu reproductor";
  if (trackArtist.textContent !== newArtist) trackArtist.textContent = newArtist;
  const newAlbum = state.album || "";
  if (trackAlbum.textContent !== newAlbum) trackAlbum.textContent = newAlbum;
  const newBadge = formatSourceApp(state.source_app_id);
  if (badgeName.textContent !== newBadge) badgeName.textContent = newBadge;

  // Play / Pause Icon (Respect optimistic state if recently clicked)
  const isPlaying = isOptimisticPlaying !== null ? isOptimisticPlaying : state.is_playing;
  if (isPlaying) {
    iconPlay.classList.add("hidden");
    iconPause.classList.remove("hidden");
  } else {
    iconPlay.classList.remove("hidden");
    iconPause.classList.add("hidden");
  }

  // Clear optimistic override once backend catches up
  if (isOptimisticPlaying !== null && state.is_playing === isOptimisticPlaying) {
    isOptimisticPlaying = null;
  }

  // Artwork & Ambient Glow (art arrives async via get_media_thumbnail; text never waits for it)
  const tkey = trackKeyOf(state);
  if (tkey !== lastTrackKey) lastTrackKey = tkey;
  if (state.thumbnail_base64) {
    if (state.thumbnail_base64 !== lastThumbnailBase64) {
      lastThumbnailBase64 = state.thumbnail_base64;
      albumArt.src = state.thumbnail_base64;
      albumArt.classList.add("visible");
      artPlaceholder.classList.add("hidden");
      applyAccentFromArt(state.thumbnail_base64);
      if (appConfig && appConfig.ambient_glow) {
        setAmbientGlow(state.thumbnail_base64);
      }
    }
    thumbFetchKey = "";
  } else if (tkey !== thumbFetchKey) {
    // New track without cached art: placeholder now, fetch once async (no spin on failure)
    thumbFetchKey = tkey;
    albumArt.removeAttribute("src");
    albumArt.classList.remove("visible");
    artPlaceholder.classList.remove("hidden");
    setAmbientGlow(null);
    resetAccent();
    lastThumbnailBase64 = null;
    invoke("get_media_thumbnail").then((b64) => {
      if (b64 && lastTrackKey === tkey) {
        lastThumbnailBase64 = b64;
        albumArt.src = b64;
        albumArt.classList.add("visible");
        artPlaceholder.classList.add("hidden");
        applyAccentFromArt(b64);
        if (appConfig && appConfig.ambient_glow) {
          setAmbientGlow(b64);
        }
      }
    }).catch(() => {});
  }

  // Timeline / Scrubber
  currentDuration = state.duration_sec || 0;
  currentIsPlaying = !!state.is_playing;
  if (!isDraggingScrubber) {
    currentPosition = state.position_sec || 0;
    timeCurrent.textContent = formatTime(currentPosition);
    timeTotal.textContent = formatTime(currentDuration);

    const pct = currentDuration > 0 ? (currentPosition / currentDuration) * 100 : 0;
    const clampedPct = Math.min(100, Math.max(0, pct));
    scrubberFill.style.width = `${clampedPct}%`;
    scrubberHead.style.left = `${clampedPct}%`;
  }
  // Only run the smooth ticker while actually playing
  if (currentIsPlaying && currentDuration > 0) startTimelineTicker();
}

// One-shot refresh right after switching source (don't wait for the 350ms poll)
async function refreshMediaState() {
  try {
    const state = await invoke("get_media_state");
    updateMediaUI(state);
  } catch (err) {
    console.error("Refresh state error:", err);
  }
}

// Controls: Play/Pause with Optimistic instant UI update
btnPlayPause.addEventListener("click", async () => {
  const currentlyPlaying = !iconPlay.classList.contains("hidden") ? false : true;
  isOptimisticPlaying = !currentlyPlaying;

  // Toggle icon immediately for responsive tactile feel
  if (isOptimisticPlaying) {
    iconPlay.classList.add("hidden");
    iconPause.classList.remove("hidden");
  } else {
    iconPlay.classList.remove("hidden");
    iconPause.classList.add("hidden");
  }

  try {
    await invoke("media_play_pause");
  } catch (err) {
    console.error("PlayPause error:", err);
    isOptimisticPlaying = null; // revert on error
  }
});

btnPrev.addEventListener("click", async () => {
  try {
    await invoke("media_previous");
  } catch (err) {
    console.error("Previous error:", err);
  }
});

btnNext.addEventListener("click", async () => {
  try {
    await invoke("media_next");
  } catch (err) {
    console.error("Next error:", err);
  }
});

btnHide.addEventListener("click", async () => {
  try {
    await invoke("hide_window");
  } catch (err) {
    console.error("Hide error:", err);
  }
});

// Master Volume Controls & Slider
function updateVolumeUI(level, muted) {
  const clamped = Math.max(0, Math.min(1, level));
  currentMasterVolume = clamped;
  isMasterMuted = !!muted;

  if (isMasterMuted) {
    iconVolHigh.classList.add("hidden");
    iconVolMute.classList.remove("hidden");
    volLabel.textContent = "Mute";
    volFill.style.width = "0%";
    volHead.style.left = "0%";
  } else {
    iconVolHigh.classList.remove("hidden");
    iconVolMute.classList.add("hidden");
    const pct = Math.round(clamped * 100);
    volLabel.textContent = `${pct}%`;
    volFill.style.width = `${pct}%`;
    volHead.style.left = `${pct}%`;
  }
}

async function syncMasterVolume() {
  try {
    const vol = await invoke("get_master_volume");
    const muted = await invoke("is_master_muted");
    updateVolumeUI(vol, muted);
  } catch (err) {
    console.error("Volume sync error:", err);
  }
}

function handleVolumeBarInput(e) {
  const rect = volBar.getBoundingClientRect();
  const offsetX = e.clientX - rect.left;
  const ratio = Math.max(0, Math.min(1, offsetX / rect.width));
  updateVolumeUI(ratio, false);
  return ratio;
}

// Throttle system-volume IPCs (~70ms) while keeping the local UI instant
const volThrottle = { last: 0, timer: null, pending: null };

function pushMasterVolume(level) {
  volThrottle.pending = level;
  const now = performance.now();
  const elapsed = now - volThrottle.last;
  if (elapsed >= 70) {
    volThrottle.last = now;
    const v = volThrottle.pending;
    volThrottle.pending = null;
    invoke("set_master_volume", { level: v }).catch((err) => console.error("Set volume error:", err));
  } else if (!volThrottle.timer) {
    volThrottle.timer = setTimeout(() => {
      volThrottle.timer = null;
      volThrottle.last = performance.now();
      const v = volThrottle.pending;
      volThrottle.pending = null;
      if (v !== null) {
        invoke("set_master_volume", { level: v }).catch((err) => console.error("Set volume error:", err));
      }
    }, 70 - elapsed);
  }
}

function flushMasterVolume() {
  if (volThrottle.timer) {
    clearTimeout(volThrottle.timer);
    volThrottle.timer = null;
  }
  if (volThrottle.pending !== null) {
    const v = volThrottle.pending;
    volThrottle.pending = null;
    volThrottle.last = performance.now();
    invoke("set_master_volume", { level: v }).catch((err) => console.error("Set volume error:", err));
  }
}

volBar.addEventListener("mousedown", (e) => {
  isDraggingVolume = true;
  pushMasterVolume(handleVolumeBarInput(e));

  const onMouseMove = (moveEvt) => {
    if (isDraggingVolume) {
      pushMasterVolume(handleVolumeBarInput(moveEvt));
    }
  };

  const onMouseUp = (upEvt) => {
    if (isDraggingVolume) {
      isDraggingVolume = false;
      pushMasterVolume(handleVolumeBarInput(upEvt));
      flushMasterVolume();
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    }
  };

  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
});

btnMute.addEventListener("click", async () => {
  try {
    const newMute = await invoke("toggle_master_mute");
    isMasterMuted = newMute;
    updateVolumeUI(currentMasterVolume, isMasterMuted);
  } catch (err) {
    console.error("Toggle mute error:", err);
  }
});

// Mouse Wheel Volume Adjustment
function showVolumeToast(delta) {
  const icon = delta > 0 ? "🔊" : "🔉";
  const action = delta > 0 ? "Subiendo" : "Bajando";
  const pct = Math.round(currentMasterVolume * 100);
  volumeToast.textContent = `${icon} ${action} volumen (${pct}%)`;
  volumeToast.classList.add("visible");

  if (volumeToastTimeout) clearTimeout(volumeToastTimeout);
  volumeToastTimeout = setTimeout(() => {
    volumeToast.classList.remove("visible");
  }, 900);
}

liquidCard.addEventListener("wheel", (e) => {
  // Ignore scrolling if inside settings drawer or source popover
  if (e.target.closest(".settings-panel") || e.target.closest(".source-popover")) return;

  e.preventDefault();
  const delta = e.deltaY < 0 ? 0.04 : -0.04;
  const newLevel = Math.max(0, Math.min(1, currentMasterVolume + delta));
  updateVolumeUI(newLevel, false);
  showVolumeToast(delta > 0 ? 1 : -1);
  pushMasterVolume(newLevel);
}, { passive: false });

// Audio Source Selector Popover
async function toggleSourcePopover(open) {
  isSourcePopoverOpen = open !== undefined ? open : !isSourcePopoverOpen;
  if (isSourcePopoverOpen) {
    if (isSettingsOpen) await toggleSettings(false);
    liquidCard.classList.add("popover-open");
    appBadge.classList.add("open");
    sourcePopover.classList.remove("hidden");
    await setWindowSize(480, 260);
    await loadAudioSources();
  } else {
    liquidCard.classList.remove("popover-open");
    appBadge.classList.remove("open");
    sourcePopover.classList.add("hidden");
    if (!isSettingsOpen) {
      await setWindowSize(480, 188);
    }
  }
}

async function loadAudioSources() {
  try {
    const sources = await invoke("get_audio_sources");
    sourceList.innerHTML = "";

    // Automatic Windows option
    const autoItem = document.createElement("div");
    const isAutoActive = selectedSourceId === null;
    autoItem.className = `source-item ${isAutoActive ? "active" : ""}`;
    autoItem.innerHTML = `
      <div class="source-item-info">
        <span class="source-item-name">Automático (Windows 11)</span>
        <span class="source-item-sub">Sesión prioritaria del sistema</span>
      </div>
      ${isAutoActive ? '<span class="source-item-badge">Activo</span>' : ""}
    `;
    autoItem.addEventListener("click", async (e) => {
      e.stopPropagation();
      selectedSourceId = null;
      try {
        await invoke("select_audio_source", { sourceId: null });
      } catch (err) {
        console.error("Select source error:", err);
      }
      void refreshMediaState();
      void syncMasterVolume();
      void toggleSourcePopover(false);
    });
    sourceList.appendChild(autoItem);

    // Active session options
    sources.forEach((s) => {
      const item = document.createElement("div");
      const isSelected = selectedSourceId !== null && selectedSourceId === s.id;
      item.className = `source-item ${isSelected ? "active" : ""}`;
      const subText = s.title && s.title !== "Sin título"
        ? `${s.title}${s.artist ? " • " + s.artist : ""}`
        : "Sesión activa detectada";
      
      const playBadge = s.is_playing ? '<span class="source-item-tag playing">▶ Reproduciendo</span>' : (s.is_current ? '<span class="source-item-tag">En curso</span>' : '');
      const activeBadge = isSelected ? '<span class="source-item-badge">Seleccionado</span>' : playBadge;

      item.innerHTML = `
        <div class="source-item-info">
          <span class="source-item-name">${escapeHtml(s.name)}</span>
          <span class="source-item-sub">${escapeHtml(subText)}</span>
        </div>
        ${activeBadge}
      `;
      item.addEventListener("click", async (e) => {
        e.stopPropagation();
        selectedSourceId = s.id;
        badgeName.textContent = s.name;
        try {
          await invoke("select_audio_source", { sourceId: s.id });
        } catch (err) {
          console.error("Select source error:", err);
        }
        void refreshMediaState();
        void syncMasterVolume();
        void toggleSourcePopover(false);
      });
      sourceList.appendChild(item);
    });

    if (sources.length === 0) {
      const empty = document.createElement("div");
      empty.className = "source-item";
      empty.innerHTML = `<span class="source-item-sub">No hay otros reproductores detectados</span>`;
      sourceList.appendChild(empty);
    }
  } catch (err) {
    console.error("Error loading sources:", err);
  }
}

appBadge.addEventListener("click", (e) => {
  e.stopPropagation();
  toggleSourcePopover();
});

btnCloseSourcePopover.addEventListener("click", (e) => {
  e.stopPropagation();
  toggleSourcePopover(false);
});

// Click outside to close source popover
document.addEventListener("click", (e) => {
  if (isSourcePopoverOpen && !e.target.closest("#sourcePopover") && !e.target.closest("#appBadge")) {
    toggleSourcePopover(false);
  }
});

// Scrubber seeking interaction
function handleScrubberMove(e) {
  const rect = scrubber.getBoundingClientRect();
  const offsetX = e.clientX - rect.left;
  const ratio = Math.min(1, Math.max(0, offsetX / rect.width));
  const targetSec = ratio * currentDuration;

  scrubberFill.style.width = `${ratio * 100}%`;
  scrubberHead.style.left = `${ratio * 100}%`;
  timeCurrent.textContent = formatTime(targetSec);
  return targetSec;
}

scrubber.addEventListener("mousedown", (e) => {
  if (currentDuration <= 0) return;
  isDraggingScrubber = true;
  handleScrubberMove(e);

  const onMouseMove = (moveEvt) => {
    if (isDraggingScrubber) {
      handleScrubberMove(moveEvt);
    }
  };

  const onMouseUp = async (upEvt) => {
    if (isDraggingScrubber) {
      isDraggingScrubber = false;
      const targetSec = handleScrubberMove(upEvt);
      try {
        await invoke("media_seek", { seconds: targetSec });
      } catch (err) {
        console.error("Seek error:", err);
      }
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    }
  };

  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
});

// Settings Drawer Toggle (guarded: rapid clicks must not stack window resizes)
async function toggleSettings(open) {
  const target = open !== undefined ? open : !isSettingsOpen;
  if (settingsBusy || target === isSettingsOpen) return;
  settingsBusy = true;
  isSettingsOpen = target;
  try {
    if (isSettingsOpen) {
      if (isSourcePopoverOpen) await toggleSourcePopover(false);
      // Only re-read config when something changed it (dragging)
      if (configDirty) {
        await loadConfigUI();
        configDirty = false;
      }
      settingsPanel.classList.remove("hidden");
      btnSettings.style.transform = "rotate(60deg)";
      btnSettings.style.color = "var(--accent-color)";
      // Fit the window to the actual panel height (DPI/font independent, avoids clipping)
      settingsPanel.style.maxHeight = "none";
      const needed = Math.ceil(liquidCard.getBoundingClientRect().height) + 18;
      const max = (window.screen.availHeight || 1080) - 40;
      const height = Math.max(188, Math.min(needed, max));
      if (needed > max) {
        // Not enough screen: cap the panel so it scrolls instead of overflowing
        const panelH = settingsPanel.getBoundingClientRect().height;
        const other = needed - 18 - panelH;
        settingsPanel.style.maxHeight = `${Math.max(140, max - other - 18)}px`;
      }
      await setWindowSize(480, height);
    } else {
      settingsPanel.classList.add("hidden");
      settingsPanel.style.maxHeight = "";
      await setWindowSize(480, 188);
      btnSettings.style.transform = "rotate(0deg)";
      btnSettings.style.color = "";
    }
  } finally {
    settingsBusy = false;
  }
}

btnSettings.addEventListener("click", () => toggleSettings());
btnCloseSettings.addEventListener("click", () => toggleSettings(false));

// Hotkey selection & badge update
function getSelectedModifiers() {
  const mods = [];
  modBtns.forEach((btn) => {
    if (btn.classList.contains("active")) {
      mods.push(btn.dataset.mod);
    }
  });
  return mods;
}

function updateBindBadge() {
  const mods = getSelectedModifiers();
  const key = keySelect.value;
  const modLabels = mods.map((m) => {
    if (m === "Control") return "Ctrl";
    return m;
  });
  const keyLabel = key === "Space" ? "Espacio" : key;
  bindBadge.textContent = `${modLabels.join(" + ")} + ${keyLabel}`;
}

modBtns.forEach((btn) => {
  btn.addEventListener("click", () => {
    btn.classList.toggle("active");
    if (getSelectedModifiers().length === 0) {
      btn.classList.add("active");
    }
    updateBindBadge();
  });
});

keySelect.addEventListener("change", updateBindBadge);

// Position presets buttons
posOptions.forEach((btn) => {
  btn.addEventListener("click", () => {
    posOptions.forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
  });
});

function getSelectedPosition() {
  const active = document.querySelector(".pos-option.active");
  if (active) return active.dataset.pos;
  // No preset selected (e.g. free position from dragging): keep the current one
  return (appConfig && appConfig.position_preset) || "top-center";
}

// Enable/disable dragging: custom JS drag + Tauri's native data-tauri-drag-region
function applyDraggable(enabled) {
  liquidCard.classList.toggle("no-drag", !enabled);
  document.querySelectorAll("[data-tauri-drag-region]").forEach((el) => {
    el.setAttribute("data-tauri-drag-region", enabled ? "" : "false");
  });
}

// Load Configuration into UI
async function loadConfigUI() {
  try {
    appConfig = await invoke("get_app_config");

    // Modifiers
    modBtns.forEach((btn) => {
      const isMod = appConfig.hotkey_modifiers.some(
        (m) => m.toLowerCase() === btn.dataset.mod.toLowerCase()
      );
      btn.classList.toggle("active", isMod);
    });

    // Key
    keySelect.value = appConfig.hotkey_key || "Space";
    updateBindBadge();

    // Position
    posOptions.forEach((btn) => {
      btn.classList.toggle("active", btn.dataset.pos === appConfig.position_preset);
    });

    // Toggles
    chkAutoHide.checked = !!appConfig.auto_hide_on_blur;
    chkAmbientGlow.checked = appConfig.ambient_glow !== false;
    chkHoverReveal.checked = !!appConfig.hover_reveal;
    chkAppVolume.checked = appConfig.app_volume !== false;
    chkDraggable.checked = appConfig.draggable !== false;
    applyDraggable(appConfig.draggable !== false);
    chkRespectFullscreen.checked = appConfig.respect_fullscreen !== false;

    if (!chkAmbientGlow.checked) {
      disableAmbientGlow();
    }
  } catch (err) {
    console.error("Error loading config:", err);
  }
}

// Save Configuration
btnSaveConfig.addEventListener("click", async () => {
  const newConfig = {
    hotkey_modifiers: getSelectedModifiers(),
    hotkey_key: keySelect.value,
    position_preset: getSelectedPosition(),
    custom_x: appConfig ? appConfig.custom_x : 100,
    custom_y: appConfig ? appConfig.custom_y : 100,
    auto_hide_on_blur: chkAutoHide.checked,
    ambient_glow: chkAmbientGlow.checked,
    volume_step: appConfig && Number.isFinite(appConfig.volume_step) ? appConfig.volume_step : 0.05,
    hover_reveal: chkHoverReveal.checked,
    app_volume: chkAppVolume.checked,
    draggable: chkDraggable.checked,
    respect_fullscreen: chkRespectFullscreen.checked,
  };

  try {
    await invoke("update_app_config", { newConfig });
    appConfig = newConfig;
    configDirty = false;
    applyDraggable(chkDraggable.checked);
    // Ambient glow toggle takes effect immediately
    if (chkAmbientGlow.checked) {
      if (lastThumbnailBase64) setAmbientGlow(lastThumbnailBase64);
    } else {
      disableAmbientGlow();
    }
    // Volume target may have switched between app/system
    void syncMasterVolume();

    // Button feedback
    const originalText = btnSaveConfig.innerHTML;
    btnSaveConfig.innerHTML = "<span>✓ Guardado con éxito</span>";
    btnSaveConfig.style.background = "#30D158";

    setTimeout(() => {
      btnSaveConfig.innerHTML = originalText;
      btnSaveConfig.style.background = "";
      toggleSettings(false);
    }, 900);
  } catch (err) {
    console.error("Save config error:", err);
  }
});

// Escape key to hide
window.addEventListener("keydown", async (e) => {
  if (e.key === "Escape") {
    if (isSourcePopoverOpen) {
      toggleSourcePopover(false);
    } else if (isSettingsOpen) {
      toggleSettings(false);
    } else {
      await invoke("hide_window");
    }
  }
});

// Native window dragging via Tauri start_drag command
document.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return; // Only primary mouse button

  // Dragging disabled by the user setting
  if (appConfig && appConfig.draggable === false) return;

  // Never initiate drag if interacting with buttons, inputs, sliders, popovers, etc.
  if (e.target.closest("button, input, select, .scrubber, .vol-bar, .source-item, .pos-option, .mod-btn, .checkbox-label, .source-popover")) {
    return;
  }

  // Allow dragging on header, metadata wrapper, background card, island
  if (
    e.target.closest("[data-tauri-drag-region]") ||
    e.target.closest(".card-header") ||
    e.target.closest(".metadata-wrapper") ||
    e.target.closest(".island-indicator") ||
    e.target.closest(".art-container") ||
    e.target === liquidCard
  ) {
    // A drag may change the saved position; refresh config next time settings opens
    configDirty = true;
    invoke("start_drag").catch((err) => {
      console.warn("start_drag error:", err);
    });
  }
});

// No browser context menu anywhere in the overlay
window.addEventListener("contextmenu", (e) => e.preventDefault());

// Smooth timeline between backend polls (only while playing, DOM at ~15fps)
const TICK_DOM_MS = 66;
let lastTickTime = performance.now();
let lastDomTick = 0;
let ticking = false;

function timelineTick(now) {
  const dt = (now - lastTickTime) / 1000;
  lastTickTime = now;
  if (!isDraggingScrubber && currentIsPlaying && currentDuration > 0) {
    currentPosition = Math.min(currentDuration, currentPosition + dt);
    if (now - lastDomTick >= TICK_DOM_MS) {
      lastDomTick = now;
      timeCurrent.textContent = formatTime(currentPosition);
      const pct = Math.min(100, Math.max(0, (currentPosition / currentDuration) * 100));
      scrubberFill.style.width = `${pct}%`;
      scrubberHead.style.left = `${pct}%`;
    }
  }
  if (currentIsPlaying && currentDuration > 0) {
    requestAnimationFrame(timelineTick);
  } else {
    ticking = false; // idle: stop scheduling entirely
  }
}

function startTimelineTicker() {
  if (ticking) return;
  ticking = true;
  lastTickTime = performance.now();
  requestAnimationFrame(timelineTick);
}

// Initialization
async function init() {
  await loadConfigUI();
  await syncMasterVolume();

  // Initial media state request
  try {
    const initialState = await invoke("get_media_state");
    updateMediaUI(initialState);
  } catch (err) {
    console.error("Error fetching initial state:", err);
  }

  // Real-time listener from Rust background thread
  await listen("media-state", (event) => {
    updateMediaUI(event.payload);
  });

  // Dynamic Island reveal/hide animation (driven by the backend hover thread)
  await listen("island-visibility", (event) => {
    if (event.payload) {
      // Ensure a frame paints in the "out" state so the transition actually plays
      requestAnimationFrame(() => liquidCard.classList.remove("island-out"));
    } else {
      liquidCard.classList.add("island-out");
    }
  });

  // Safety: never leave the card faded out when the window is focused/shown (hotkey, click)
  try {
    const appWindow = window.__TAURI__.window.getCurrentWindow();
    await appWindow.onFocusChanged(({ payload: focused }) => {
      if (focused) liquidCard.classList.remove("island-out");
    });
  } catch (err) {
    console.warn("Focus listener unavailable:", err);
  }

  // Safety: if the pointer enters the window, it must be interactive (covers focus-steal cases)
  document.addEventListener("mouseenter", () => liquidCard.classList.remove("island-out"));
}

window.addEventListener("DOMContentLoaded", init);
