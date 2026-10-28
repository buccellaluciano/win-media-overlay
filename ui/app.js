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
const subSep = document.getElementById("subSep");
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
const btnHoverReveal = document.getElementById("btnHoverReveal");
const btnLyricsToggle = document.getElementById("btnLyricsToggle");
const iconLyricsChevron = document.getElementById("iconLyricsChevron");
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
const chkAutoHide = document.getElementById("chkAutoHide");
const chkAmbientGlow = document.getElementById("chkAmbientGlow");
const chkAppVolume = document.getElementById("chkAppVolume");
const chkDraggable = document.getElementById("chkDraggable");
const chkRespectFullscreen = document.getElementById("chkRespectFullscreen");
const chkLyrics = document.getElementById("chkLyrics");
const volumeStepSelect = document.getElementById("volumeStepSelect");
const bindError = document.getElementById("bindError");
const chkIslandEnabled = document.getElementById("chkIslandEnabled");
const chkIslandAlwaysVisible = document.getElementById("chkIslandAlwaysVisible");
const chkIslandActivities = document.getElementById("chkIslandActivities");
const expandDelaySelect = document.getElementById("expandDelaySelect");
const statsStrip = document.getElementById("statsStrip");
const chkStatsEnabled = document.getElementById("chkStatsEnabled");
const statsIntervalSelect = document.getElementById("statsIntervalSelect");
const chkStatCpu = document.getElementById("chkStatCpu");
const chkStatGpu = document.getElementById("chkStatGpu");
const chkStatRam = document.getElementById("chkStatRam");
const chkStatVram = document.getElementById("chkStatVram");
const chkStatNet = document.getElementById("chkStatNet");
const chkStatDisk = document.getElementById("chkStatDisk");
const chkStatGpuTemp = document.getElementById("chkStatGpuTemp");
const chkStatsInIsland = document.getElementById("chkStatsInIsland");
const chkStatsAlerts = document.getElementById("chkStatsAlerts");
const chkNotifEnabled = document.getElementById("chkNotifEnabled");
const chkNotifShowBody = document.getElementById("chkNotifShowBody");
const notifDurationSelect = document.getElementById("notifDurationSelect");
const notifIgnoreInput = document.getElementById("notifIgnoreInput");
const notifAccess = document.getElementById("notifAccess");
const btnNotifTest = document.getElementById("btnNotifTest");

// Appearance elements
const themeGrid = document.getElementById("themeGrid");
const themeSwatches = document.querySelectorAll(".theme-swatch");
const chkAccentFromArt = document.getElementById("chkAccentFromArt");
const cardOpacityRange = document.getElementById("cardOpacityRange");
const cardOpacityValue = document.getElementById("cardOpacityValue");

// Dynamic Island elements
const expandedView = document.getElementById("expandedView");
const islandCompact = document.getElementById("islandCompact");
const islandArt = document.getElementById("islandArt");
const islandTitle = document.getElementById("islandTitle");
const islandTitleBox = document.getElementById("islandTitleBox");
const islandEq = document.getElementById("islandEq");
const islandActivity = document.getElementById("islandActivity");
const islandActivityArt = document.getElementById("islandActivityArt");
const islandActivityLabel = document.getElementById("islandActivityLabel");
const islandActivityTitle = document.getElementById("islandActivityTitle");

// Lyrics Elements
const lyricsPanel = document.getElementById("lyricsPanel");
const lyricsScroll = document.getElementById("lyricsScroll");
const lyricsLines = document.getElementById("lyricsLines");

// State
let isDraggingScrubber = false;
let currentDuration = 0;
let currentPosition = 0;
let currentIsPlaying = false;
let isSettingsOpen = false;
let settingsBusy = false;
let configDirty = false;
let islandMode = ""; // compact | activity | expanded | hidden, owned by the backend
let lastMediaState = null;
let lastIslandTrackKey = "";
let lastIslandSource = "";
let lastStats = null;
let statNodes = {};
let statsBuiltKey = "";
let islandStatsAlt = false;
const statsAlertSince = { cpu: null, gpu: null, ram: null };
const statsLastAlert = { cpu: 0, gpu: 0, ram: 0 };
let themeChoice = "liquid";
let themePreview = "liquid";
let resolvedTheme = "liquid";
let opacityPreview = 0.74;
let isSourcePopoverOpen = false;
let selectedSourceId = null;
let currentMasterVolume = 0.5;
let isMasterMuted = false;
let isDraggingVolume = false;
let appConfig = null;
let lastThumbnailBase64 = null;
let lastTrackKey = "";
let thumbFetchKey = "";
let lastVolumeTrackKey = ""; // detect track changes to re-apply the per-app volume
let glowFront = null; // layer currently visible (A/B), the other cross-fades in

// Lyrics state
let lyricsLinesData = []; // [{t, text}]
let lyricsTrackKey = "";
let lyricsActiveIdx = -1;
let lyricsRequestKey = "";
let lyricsAvailable = false; // there are synced lyrics for the current track
let lyricsCollapsed = false; // user collapsed the panel with the chevron
const LYRICS_CACHE = new Map(); // trackKey -> lines[] | null
const LYRICS_CACHE_LIMIT = 200;

// Bounded cache: drop the oldest entry instead of growing forever
function cacheLyrics(key, value) {
  if (!LYRICS_CACHE.has(key) && LYRICS_CACHE.size >= LYRICS_CACHE_LIMIT) {
    const oldest = LYRICS_CACHE.keys().next().value;
    if (oldest !== undefined) LYRICS_CACHE.delete(oldest);
  }
  LYRICS_CACHE.set(key, value);
}

// Cross-fade the ambient glow: set the new art on the hidden layer, then fade layers
function setAmbientGlow(src) {
  if (glowFront === null) glowFront = ambientGlowA;
  const back = glowFront === ambientGlowA ? ambientGlowB : ambientGlowA;
  if (src) {
    back.style.backgroundImage = `url(${src})`;
    back.style.opacity = "var(--glow-strength)";
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
  if (!appConfig || appConfig.accent_from_art === false || THEME_FIXED_ACCENT[resolvedTheme]) {
    return;
  }
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

// =========================================================
// Themes (Fase 4)
// =========================================================

const THEME_IDS = [
  "liquid",
  "frost",
  "oled",
  "mica",
  "neon",
  "sunset",
  "minimal",
  "contrast",
  "auto",
];

// Themes whose accent is part of the design and must ignore the album art
const THEME_FIXED_ACCENT = {
  frost: true,
  mica: true,
  sunset: true,
  minimal: true,
  contrast: true,
};

function resolveTheme(id) {
  if (id === "auto") {
    return window.matchMedia && window.matchMedia("(prefers-color-scheme: light)").matches
      ? "frost"
      : "liquid";
  }
  return THEME_IDS.includes(id) ? id : "liquid";
}

function refreshAccent() {
  const useArt =
    !!(appConfig && appConfig.accent_from_art !== false) && !THEME_FIXED_ACCENT[resolvedTheme];
  if (useArt && lastThumbnailBase64) {
    applyAccentFromArt(lastThumbnailBase64);
  } else {
    resetAccent();
  }
}

function updateThemeSwatches(id) {
  themeSwatches.forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.theme === id);
  });
}

function applyTheme(id) {
  const choice = THEME_IDS.includes(id) ? id : "liquid";
  resolvedTheme = resolveTheme(choice);
  document.documentElement.dataset.theme = resolvedTheme;
  document.documentElement.dataset.themeChoice = choice;
  updateThemeSwatches(choice);
  refreshAccent();
}

function applyOpacity(value) {
  const v = Math.max(0.4, Math.min(1, Number(value) || 0.74));
  opacityPreview = v;
  document.documentElement.style.setProperty("--card-opacity-user", String(v));
  if (cardOpacityValue) cardOpacityValue.textContent = `${Math.round(v * 100)} %`;
  if (cardOpacityRange && Math.abs(Number(cardOpacityRange.value) - v) > 0.001) {
    cardOpacityRange.value = String(v);
  }
}

function trackKeyOf(state) {
  return `${state.source_app_id || ""}::${state.title || ""}::${state.artist || ""}`;
}

// =========================================================
// Synced lyrics (LRCLIB, opt-in). Only syncedLyrics are used.
// =========================================================

function parseLrc(text) {
  if (!text) return [];
  const out = [];
  const re = /\[(\d{1,2}):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;
  for (const raw of text.split(/\r?\n/)) {
    const stamps = [];
    let m;
    re.lastIndex = 0;
    while ((m = re.exec(raw)) !== null) {
      const min = parseInt(m[1], 10);
      const sec = parseInt(m[2], 10);
      const frac = m[3] ? parseInt(m[3].padEnd(3, "0"), 10) / 1000 : 0;
      stamps.push(min * 60 + sec + frac);
    }
    if (!stamps.length) continue;
    const content = raw.replace(re, "").trim();
    for (const t of stamps) out.push({ t, text: content });
  }
  out.sort((a, b) => a.t - b.t);
  // Cap line count: a hostile/huge response must not blow up the DOM
  return out.length > 5000 ? out.slice(0, 5000) : out;
}

// Read a JSON response with a size cap (defends against a huge/malicious body)
async function readJsonCapped(res) {
  if (!res || !res.ok) return null;
  const text = await res.text();
  if (text.length > 2_000_000) return null;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

async function fetchLyrics(state) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 6000);
  const params = new URLSearchParams({
    track_name: state.title || "",
    artist_name: state.artist || "",
    album_name: state.album || "",
    duration: String(Math.round(state.duration_sec || 0)),
  });
  try {
    const res = await fetch(`https://lrclib.net/api/get?${params}`, { signal: controller.signal });
    if (res.ok) {
      const data = await readJsonCapped(res);
      return data && data.syncedLyrics ? parseLrc(data.syncedLyrics) : null;
    }
    // Fallback: fuzzy search
    const q = new URLSearchParams({
      track_name: state.title || "",
      artist_name: state.artist || "",
    });
    const arr = await readJsonCapped(
      await fetch(`https://lrclib.net/api/search?${q}`, { signal: controller.signal })
    );
    const hit = Array.isArray(arr) ? arr.find((r) => r && r.syncedLyrics) : null;
    return hit ? parseLrc(hit.syncedLyrics) : null;
  } finally {
    clearTimeout(timeout);
  }
}

function renderLyrics(lines) {
  lyricsLines.innerHTML = "";
  lyricsActiveIdx = -1;
  const frag = document.createDocumentFragment();
  lines.forEach((line, i) => {
    const div = document.createElement("div");
    div.className = "lyric-line";
    div.textContent = line.text || "♪"; // textContent: never innerHTML with remote text
    div.dataset.idx = String(i);
    frag.appendChild(div);
  });
  lyricsLines.appendChild(frag);
}

// Panel is shown when lyrics exist, the user hasn't collapsed them, and no other panel is open
function lyricsShouldShow() {
  return lyricsAvailable && !lyricsCollapsed;
}

function applyLyricsPanelDom() {
  const show = lyricsShouldShow() && !isSettingsOpen && !isSourcePopoverOpen;
  lyricsPanel.classList.toggle("hidden", !show);
}

// Chevron only appears when there are lyrics; points up (collapse) when expanded, down when collapsed
function updateLyricsChevron() {
  btnLyricsToggle.classList.toggle("hidden", !lyricsAvailable);
  const expanded = lyricsShouldShow();
  btnLyricsToggle.classList.toggle("active", expanded);
  iconLyricsChevron.style.transform = expanded ? "rotate(0deg)" : "rotate(180deg)";
}

// The native window is a fixed transparent box (it never resizes: that flashed a shifted
// frame). Tell the backend where the card is so everything else stays click-through.
let pointerCapture = false; // a slider drag may leave the card: keep the mouse meanwhile
let lastHitKey = "";

function reportHitRect() {
  const r = liquidCard.getBoundingClientRect();
  const key = `${Math.round(r.left)},${Math.round(r.top)},${Math.round(r.width)},${Math.round(r.height)},${pointerCapture}`;
  if (key === lastHitKey) return;
  lastHitKey = key;
  invoke("set_hit_rect", {
    x: r.left,
    y: r.top,
    width: r.width,
    height: r.height,
    capture: pointerCapture,
  }).catch(() => {});
}

// Every size change (morphs, settings, lyrics, stats) reports itself; no manual calls needed
new ResizeObserver(reportHitRect).observe(liquidCard);
// Safety net if a resize callback is ever skipped (throttled WebView): cheap, deduped
setInterval(reportHitRect, 1000);

document.addEventListener("pointerdown", (e) => {
  if (e.target.closest(".scrubber, .vol-bar, input[type='range']")) {
    pointerCapture = true;
    reportHitRect();
  }
});
["pointerup", "pointercancel"].forEach((type) =>
  window.addEventListener(type, () => {
    if (!pointerCapture) return;
    pointerCapture = false;
    reportHitRect();
  })
);

function setLyricsAvailable(available) {
  lyricsAvailable = !!available;
  if (!lyricsAvailable) {
    lyricsLines.innerHTML = "";
    lyricsLinesData = [];
    lyricsActiveIdx = -1;
  }
  updateLyricsChevron();
  applyLyricsPanelDom();
}

function syncLyricsToPosition() {
  if (!lyricsShouldShow() || !lyricsLinesData.length) return;
  let idx = -1;
  for (let i = 0; i < lyricsLinesData.length; i++) {
    if (lyricsLinesData[i].t <= currentPosition + 0.15) idx = i;
    else break;
  }
  if (idx === lyricsActiveIdx) return;
  lyricsActiveIdx = idx;
  const nodes = lyricsLines.children;
  for (let i = 0; i < nodes.length; i++) {
    nodes[i].classList.toggle("active", i === idx);
    nodes[i].classList.toggle("past", i < idx);
  }
  if (idx >= 0 && nodes[idx]) {
    const node = nodes[idx];
    const target = node.offsetTop - lyricsScroll.clientHeight / 2 + node.offsetHeight / 2;
    lyricsScroll.scrollTo({ top: Math.max(0, target), behavior: "smooth" });
  }
}

// Chevron: collapse/expand lyrics (only meaningful when lyrics exist)
btnLyricsToggle.addEventListener("click", (e) => {
  e.stopPropagation();
  lyricsCollapsed = !lyricsCollapsed;
  updateLyricsChevron();
  void morphExpanded(applyLyricsPanelDom);
});

function updateLyricsForState(state) {
  const enabled = appConfig && appConfig.lyrics_enabled;
  const key = trackKeyOf(state);
  if (!enabled || !state.has_session || !state.title || state.title === "Sin reproducción") {
    setLyricsAvailable(false);
    return;
  }
  if (key === lyricsTrackKey) return;
  lyricsTrackKey = key;

  if (LYRICS_CACHE.has(key)) {
    const cached = LYRICS_CACHE.get(key);
    if (cached && cached.length) {
      lyricsLinesData = cached;
      renderLyrics(cached);
      setLyricsAvailable(true);
      syncLyricsToPosition();
    } else {
      setLyricsAvailable(false);
    }
    return;
  }

  lyricsRequestKey = key;
  setLyricsAvailable(false);
  fetchLyrics(state)
    .then((lines) => {
      const good = lines && lines.length ? lines : null;
      cacheLyrics(key, good);
      // Ignore stale responses (track changed while fetching)
      if (lyricsRequestKey !== key || lyricsTrackKey !== key) return;
      if (good) {
        lyricsLinesData = good;
        renderLyrics(good);
        setLyricsAvailable(true);
        syncLyricsToPosition();
      }
    })
    .catch(() => {
      cacheLyrics(key, null);
      if (lyricsRequestKey === key) setLyricsAvailable(false);
    });
}


let isOptimisticPlaying = null;
let playOptimisticTimer = null;
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

// =========================================================
// Dynamic Island (compact / activity / expanded)
// =========================================================

const activityQueue = [];
let activityShowing = false;
let activityTimer = null;
let lastActivityKey = "";
let shownActivityKey = "";

function setIslandArt(src) {
  if (src) {
    if (islandArt.getAttribute("src") !== src) islandArt.src = src;
    islandArt.classList.add("has-art");
  } else {
    islandArt.removeAttribute("src");
    islandArt.classList.remove("has-art");
  }
}

function updateIslandCompact(state) {
  const has = !!(state && state.has_session);
  const showStats =
    islandStatsAlt &&
    islandMode === "compact" &&
    !!appConfig &&
    !!appConfig.stats_in_island &&
    !!lastStats;

  let title = has ? state.title || "Sin reproducción" : "Sin reproducción";
  if (showStats) {
    const summary = statsSummary(lastStats);
    if (summary) title = summary;
    islandTitle.classList.remove("scrollable");
  }
  if (islandTitle.textContent !== title) {
    islandTitle.textContent = title;
    if (!showStats) requestAnimationFrame(() => checkMarquee(islandTitle, islandTitleBox));
  }
  islandEq.classList.toggle("playing", !!(state && state.is_playing));
  setIslandArt(has ? state.thumbnail_base64 || lastThumbnailBase64 : null);
}

function renderIslandActivity(item) {
  islandActivityLabel.textContent = item.label;
  islandActivityTitle.textContent = item.text; // textContent: never innerHTML with metadata
  if (item.art) {
    islandActivityArt.src = item.art;
    islandActivityArt.classList.add("has-art");
  } else {
    islandActivityArt.removeAttribute("src");
    islandActivityArt.classList.remove("has-art");
  }
}

function clearActivities() {
  activityQueue.length = 0;
  activityShowing = false;
  shownActivityKey = "";
  if (activityTimer) {
    clearTimeout(activityTimer);
    activityTimer = null;
  }
}

// Queue max 3, no consecutive duplicates; only while compact
function enqueueActivity(item) {
  if (!appConfig || !appConfig.island_enabled || !appConfig.island_activities) return;
  if (islandMode !== "compact") return; // expanded already shows this info
  if (!item || !item.text) return;
  // Repeated activity (e.g. wheel volume): refresh the visible text instead of queuing
  if (activityShowing && item.key === shownActivityKey) {
    islandActivityTitle.textContent = item.text;
    lastActivityKey = item.key;
    return;
  }
  if (item.key === lastActivityKey) return;
  if (activityQueue.length >= 3) return;
  activityQueue.push(item);
  lastActivityKey = item.key;
  if (!activityShowing) void showNextActivity();
}

async function showNextActivity() {
  const item = activityQueue.shift();
  if (!item) {
    activityShowing = false;
    return;
  }
  activityShowing = true;
  shownActivityKey = item.key;
  renderIslandActivity(item);
  try {
    await invoke("request_island_mode", { mode: "activity" });
  } catch (err) {
    console.error("Island activity error:", err);
    activityShowing = false;
    return;
  }
  if (activityTimer) clearTimeout(activityTimer);
  activityTimer = setTimeout(async () => {
    activityTimer = null;
    if (activityQueue.length) {
      void showNextActivity();
      return;
    }
    activityShowing = false;
    shownActivityKey = "";
    // Allow the same activity (e.g. volume) to show again after a gap
    lastActivityKey = "";
    // The user may have expanded while the activity was showing
    if (islandMode !== "activity") return;
    try {
      await invoke("request_island_mode", { mode: "compact" });
    } catch (err) {
      console.error("Island activity end error:", err);
    }
  }, 2500);
}

// Animate the card between the island's explicit height and the expanded auto height.
// Native windows can't tween, so only the card morphs and the window snaps at the right time.
let cardMorphTimer = null;

function morphCard(prevMode, mode, collapseFrom) {
  const prevIsland = prevMode === "compact" || prevMode === "activity";
  const nextIsland = mode === "compact" || mode === "activity";
  if (!prevMode || prevMode === mode || mode === "hidden" || prevMode === "hidden") return;
  if (!prevIsland && !nextIsland) return;

  let from;
  let to;
  if (nextIsland && !prevIsland) {
    // Collapsing: height measured before the dataset switch
    from = collapseFrom;
    to = mode === "compact" ? 36 : 56;
  } else if (!nextIsland && prevIsland) {
    // Expanding: morph up to the natural height of the expanded content
    from = prevMode === "compact" ? 36 : 56;
    to = Math.max(176, expandedContentHeight());
  } else {
    from = prevMode === "compact" ? 36 : 56;
    to = mode === "compact" ? 36 : 56;
  }
  if (!from || !to) return;

  // Pin the start, force a reflow, then set the target in the same task: the transition runs.
  // (A rAF here could land after the cleanup timer when the WebView is throttled and leave the
  // card stuck at a fixed height.)
  liquidCard.style.height = `${from}px`;
  void liquidCard.offsetHeight;
  liquidCard.style.height = `${to}px`;

  if (cardMorphTimer) clearTimeout(cardMorphTimer);
  cardMorphTimer = setTimeout(() => {
    cardMorphTimer = null;
    liquidCard.style.height = ""; // back to auto in expanded / CSS height in island modes
    reportHitRect();
  }, MORPH_MS);
}

// Matches --morph-duration in style.css (+ a frame of slack)
const MORPH_MS = 300;
let expandMorphTimer = null;

// Animate the expanded card between content heights (settings, popover, lyrics). Only the card
// moves: the window is a fixed transparent box, so nothing native has to keep up.
function morphExpanded(applyChange) {
  if ((islandMode && islandMode !== "expanded") || cardMorphTimer) {
    applyChange();
    return;
  }
  if (expandMorphTimer) {
    clearTimeout(expandMorphTimer);
    expandMorphTimer = null;
  }
  const from = liquidCard.getBoundingClientRect().height;
  liquidCard.style.height = "";
  applyChange();
  const to = liquidCard.getBoundingClientRect().height;
  if (Math.abs(to - from) < 1) return;
  liquidCard.style.height = `${from}px`;
  void liquidCard.offsetHeight; // same reflow trick as morphCard (no rAF race)
  liquidCard.style.height = `${to}px`;
  expandMorphTimer = setTimeout(() => {
    expandMorphTimer = null;
    liquidCard.style.height = "";
    reportHitRect();
  }, MORPH_MS);
}

// Expanded content height. .expanded-view has a fixed width (style.css), so this is right even
// while the card is still ~240 px wide coming out of the island.
function expandedContentHeight() {
  const bw = parseFloat(getComputedStyle(liquidCard).borderTopWidth) || 0;
  return expandedView.scrollHeight + bw * 2;
}

// Single place where the frontend mirrors the backend island state
function applyIslandMode(mode) {
  if (!mode) return;
  // An island change wins over a pending settings/popover morph (its cleanup would cut this one)
  if (expandMorphTimer) {
    clearTimeout(expandMorphTimer);
    expandMorphTimer = null;
    liquidCard.style.height = "";
  }
  const prevMode = islandMode;
  const collapseFrom =
    prevMode === "expanded" && (mode === "compact" || mode === "activity")
      ? liquidCard.getBoundingClientRect().height
      : 0;

  islandMode = mode;
  document.documentElement.dataset.mode = mode;
  liquidCard.classList.remove("island-out");
  if (mode === "expanded") {
    clearActivities();
    applyLyricsPanelDom();
  } else if (mode === "hidden") {
    clearActivities();
  } else if (mode === "compact") {
    clearActivities();
    if (lastMediaState) updateIslandCompact(lastMediaState);
  }
  if (mode !== "compact") islandStatsAlt = false;

  morphCard(prevMode, mode, collapseFrom);
  reportHitRect();
}

// Settings/popover must not collapse when the cursor leaves
function updateIslandPin() {
  const pinned = !!(isSettingsOpen || isSourcePopoverOpen);
  invoke("set_island_pinned", { pinned }).catch(() => {});
}

// =========================================================
// System stats (Fase 3)
// =========================================================

const STAT_META = {
  cpu: { label: "CPU" },
  gpu: { label: "GPU" },
  ram: { label: "RAM" },
  vram: { label: "VRAM" },
  net: { label: "" },
  disk: { label: "Disco" },
  gpu_temp: { label: "Temp" },
};

function statAvailable(stats, key) {
  switch (key) {
    case "gpu":
      return stats.gpu_pct != null;
    case "vram":
      return stats.vram_used_mb != null;
    case "net":
      return stats.net_down_kbps != null;
    case "disk":
      return stats.disk_pct != null;
    case "gpu_temp":
      return stats.gpu_temp_c != null;
    default:
      return true;
  }
}

function statColor(pct) {
  if (pct >= 85) return "var(--stat-bad)";
  if (pct >= 60) return "var(--stat-warn)";
  return "var(--stat-ok)";
}

function fmtGB(mb) {
  return `${(mb / 1024).toFixed(1)} GB`;
}

function fmtRate(kbps) {
  if (!Number.isFinite(kbps)) return "0 KB/s";
  if (kbps >= 1024) return `${(kbps / 1024).toFixed(1)} MB/s`;
  if (kbps >= 10) return `${Math.round(kbps)} KB/s`;
  return `${kbps.toFixed(1)} KB/s`;
}

function statsSummary(stats) {
  const parts = [];
  if (stats && Number.isFinite(stats.cpu_pct)) parts.push(`CPU ${Math.round(stats.cpu_pct)}%`);
  if (stats && stats.gpu_pct != null) parts.push(`GPU ${Math.round(stats.gpu_pct)}%`);
  if (parts.length < 2 && stats && Number.isFinite(stats.ram_pct)) {
    parts.push(`RAM ${Math.round(stats.ram_pct)}%`);
  }
  return parts.join(" · ");
}

function buildStatsChips(keys) {
  statsStrip.innerHTML = ""; // only labels from STAT_META, never remote text
  statNodes = {};
  const frag = document.createDocumentFragment();
  for (const key of keys) {
    const meta = STAT_META[key];
    if (!meta) continue;
    const chip = document.createElement("span");
    chip.className = "stat-chip";
    if (meta.label) {
      const label = document.createElement("span");
      label.textContent = meta.label;
      chip.appendChild(label);
    }
    const value = document.createElement("b");
    value.textContent = "—";
    chip.appendChild(value);
    let bar = null;
    if (
      key === "cpu" ||
      key === "gpu" ||
      key === "ram" ||
      key === "vram" ||
      key === "disk" ||
      key === "gpu_temp"
    ) {
      const fill = document.createElement("span");
      fill.className = "stat-fill";
      bar = document.createElement("i");
      fill.appendChild(bar);
      chip.appendChild(fill);
    }
    frag.appendChild(chip);
    statNodes[key] = { chip, value, bar, color: "" };
  }
  statsStrip.appendChild(frag);
}

function setStatColor(node, color) {
  if (node.color !== color) {
    node.chip.style.setProperty("--stat-color", color);
    node.color = color;
  }
}

function updateStatChip(key, stats) {
  const node = statNodes[key];
  if (!node) return;
  let text = "";
  let pct = null;
  let color = "";

  switch (key) {
    case "cpu":
      pct = stats.cpu_pct;
      text = `${Math.round(pct)}%`;
      break;
    case "gpu":
      pct = stats.gpu_pct || 0;
      text = `${Math.round(pct)}%`;
      break;
    case "ram":
      pct = stats.ram_pct;
      text = `${fmtGB(stats.ram_used_mb)}/${Math.round(stats.ram_total_mb / 1024)} GB`;
      break;
    case "vram": {
      const total = stats.vram_total_mb;
      text = total
        ? `${fmtGB(stats.vram_used_mb)}/${Math.round(total / 1024)} GB`
        : fmtGB(stats.vram_used_mb);
      pct = total ? (stats.vram_used_mb / total) * 100 : null;
      break;
    }
    case "net":
      text = `▼ ${fmtRate(stats.net_down_kbps)}  ▲ ${fmtRate(stats.net_up_kbps)}`;
      break;
    case "disk":
      pct = stats.disk_pct;
      text = `${Math.round(pct)}%`;
      break;
    case "gpu_temp": {
      const t = stats.gpu_temp_c;
      text = `${Math.round(t)}°C`;
      pct = Math.min(100, Math.max(0, t));
      color = t >= 85 ? "var(--stat-bad)" : t >= 70 ? "var(--stat-warn)" : "var(--stat-ok)";
      break;
    }
    default:
      return;
  }

  if (node.value.textContent !== text) node.value.textContent = text;
  if (node.bar && pct !== null && Number.isFinite(pct)) {
    const width = `${Math.min(100, Math.max(0, pct)).toFixed(0)}%`;
    if (node.bar.style.width !== width) node.bar.style.width = width;
    if (!color) color = statColor(pct);
  }
  if (color) setStatColor(node, color);
}

function renderStats(stats) {
  if (!stats) return;
  lastStats = stats;
  const cfg = appConfig;
  if (!cfg || !cfg.stats_enabled) return;

  const items = Array.isArray(cfg.stats_items) ? cfg.stats_items : [];
  const keys = items.filter((key) => STAT_META[key] && statAvailable(stats, key));
  const buildKey = keys.join(",");
  if (buildKey !== statsBuiltKey) {
    buildStatsChips(keys);
    statsBuiltKey = buildKey;
    statsStrip.classList.toggle("hidden", keys.length === 0);
  }
  keys.forEach((key) => updateStatChip(key, stats));
  checkStatsAlerts(stats);
}

function resetStatsUI() {
  lastStats = null;
  statsBuiltKey = "";
  statNodes = {};
  statsStrip.innerHTML = "";
  statsStrip.classList.add("hidden");
}

function checkStatsAlerts(stats) {
  const cfg = appConfig;
  if (!cfg || !cfg.stats_enabled || !cfg.stats_alerts) return;
  const now = performance.now();
  const watch = [
    ["cpu", Number.isFinite(stats.cpu_pct) && stats.cpu_pct > 95, stats.cpu_pct],
    ["gpu", stats.gpu_pct != null && stats.gpu_pct > 95, stats.gpu_pct],
    ["ram", Number.isFinite(stats.ram_pct) && stats.ram_pct > 90, stats.ram_pct],
  ];
  for (const [key, over, value] of watch) {
    if (!over) {
      statsAlertSince[key] = null;
      continue;
    }
    if (statsAlertSince[key] === null) statsAlertSince[key] = now;
    if (now - statsAlertSince[key] >= 10000 && now - statsLastAlert[key] >= 300000) {
      statsAlertSince[key] = null;
      statsLastAlert[key] = now;
      enqueueActivity({
        key: `alert:${key}`,
        label: "Recursos",
        text: `${key.toUpperCase()} al ${Math.round(value)} %`,
        art: lastThumbnailBase64,
      });
    }
  }
}

function applyStatsConfig() {
  if (!appConfig || !appConfig.stats_enabled) resetStatsUI();
}

// Update UI with incoming MediaState
function updateMediaUI(state) {
  if (!state) return;
  lastMediaState = state;

  // Track changed: the player likely recreated its audio session (volume reset)
  const vkey = trackKeyOf(state);
  if (vkey !== lastVolumeTrackKey) {
    lastVolumeTrackKey = vkey;
    reapplyVolumeSoon();
  }

  // Dynamic Island: transient activity on track/source change
  if (!lastIslandTrackKey) {
    lastIslandTrackKey = vkey;
    lastIslandSource = state.source_app_id || "";
  } else if (vkey !== lastIslandTrackKey) {
    lastIslandTrackKey = vkey;
    if (state.has_session && state.title && state.title !== "Sin reproducción") {
      enqueueActivity({
        key: `track:${vkey}`,
        label: "Ahora suena",
        text: `${state.title}${state.artist ? " — " + state.artist : ""}`,
        art: null, // filled in when the thumbnail arrives
      });
    }
  }
  if (state.has_session && lastIslandSource && state.source_app_id !== lastIslandSource) {
    enqueueActivity({
      key: `src:${state.source_app_id}`,
      label: "Fuente de audio",
      text: state.source_name || "Windows Media",
      art: state.thumbnail_base64 || lastThumbnailBase64,
    });
  }
  lastIslandSource = state.source_app_id || "";

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
  // No album: no dangling "•" after the artist
  subSep.classList.toggle("hidden", !newAlbum);
  const newBadge = state.source_name || "Windows Media";
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
    if (playOptimisticTimer) {
      clearTimeout(playOptimisticTimer);
      playOptimisticTimer = null;
    }
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
      setIslandArt(state.thumbnail_base64);
      if (islandMode === "activity") {
        islandActivityArt.src = state.thumbnail_base64;
        islandActivityArt.classList.add("has-art");
      }
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
        setIslandArt(b64);
        if (islandMode === "activity") {
          islandActivityArt.src = b64;
          islandActivityArt.classList.add("has-art");
        }
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

  // Synced lyrics follow the track/position
  updateLyricsForState(state);

  // Compact island mirrors the same state
  updateIslandCompact(state);
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

  // Never leave the optimistic state stuck if the player ignores the toggle
  if (playOptimisticTimer) clearTimeout(playOptimisticTimer);
  playOptimisticTimer = setTimeout(() => {
    playOptimisticTimer = null;
    if (isOptimisticPlaying !== null) {
      isOptimisticPlaying = null;
      void refreshMediaState();
    }
  }, 1500);

  try {
    await invoke("media_play_pause");
  } catch (err) {
    console.error("PlayPause error:", err);
    isOptimisticPlaying = null; // revert on error
    if (playOptimisticTimer) {
      clearTimeout(playOptimisticTimer);
      playOptimisticTimer = null;
    }
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
    if (appConfig && appConfig.island_enabled) {
      // Collapse to compact instead of hiding the window out-of-band
      await invoke("request_island_mode", { mode: "compact" });
    } else {
      await invoke("hide_window");
    }
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

// Players recreate their audio session on track change/seek, which resets its volume.
// Re-apply our per-app value (backend re-enumerates current sessions) and re-sync the UI.
async function reapplyVolume() {
  try {
    await invoke("reapply_app_volume");
    await syncMasterVolume();
  } catch (err) {
    console.error("Reapply volume error:", err);
  }
}

// The new session can appear a bit after the event, so retry shortly after
function reapplyVolumeSoon() {
  void reapplyVolume();
  setTimeout(() => void reapplyVolume(), 500);
  setTimeout(() => void reapplyVolume(), 1200);
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

function volumeStep() {
  const step = appConfig && Number(appConfig.volume_step);
  return Number.isFinite(step) ? Math.min(0.2, Math.max(0.01, step)) : 0.05;
}

liquidCard.addEventListener("wheel", async (e) => {
  // Ignore scrolling if inside settings drawer or source popover
  if (e.target.closest(".settings-panel") || e.target.closest(".source-popover")) return;

  e.preventDefault();
  const step = volumeStep();
  const delta = e.deltaY < 0 ? step : -step;

  // Unmute in the backend too: otherwise the UI says "not muted" while audio stays muted
  if (isMasterMuted) {
    try {
      await invoke("set_master_muted", { muted: false });
      isMasterMuted = false;
    } catch (err) {
      console.error("Unmute error:", err);
    }
  }

  const newLevel = Math.max(0, Math.min(1, currentMasterVolume + delta));
  updateVolumeUI(newLevel, false);
  showVolumeToast(delta > 0 ? 1 : -1);
  pushMasterVolume(newLevel);

  // Compact island: the toast is hidden there, so surface the level as an activity
  if (islandMode === "compact") {
    enqueueActivity({
      key: "volume",
      label: "Volumen",
      text: `${Math.round(newLevel * 100)} %`,
      art: null,
    });
  }
}, { passive: false });

// Audio Source Selector Popover
async function toggleSourcePopover(open) {
  isSourcePopoverOpen = open !== undefined ? open : !isSourcePopoverOpen;
  updateIslandPin();
  if (isSourcePopoverOpen) {
    if (isSettingsOpen) await toggleSettings(false);
    appBadge.classList.add("open");
    await loadAudioSources();
    await morphExpanded(() => {
      lyricsPanel.classList.add("hidden");
      liquidCard.classList.add("popover-open");
      sourcePopover.classList.remove("hidden");
    });
  } else {
    appBadge.classList.remove("open");
    if (isSettingsOpen) {
      liquidCard.classList.remove("popover-open");
      sourcePopover.classList.add("hidden");
    } else {
      await morphExpanded(() => {
        liquidCard.classList.remove("popover-open");
        sourcePopover.classList.add("hidden");
        applyLyricsPanelDom();
      });
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
        // Seeking can make the player rebuild its audio session
        reapplyVolumeSoon();
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
  updateIslandPin();
  try {
    if (isSettingsOpen) {
      if (isSourcePopoverOpen) await toggleSourcePopover(false);
      // Only re-read config when something changed it (dragging)
      if (configDirty) {
        await loadConfigUI();
        configDirty = false;
      }
      btnSettings.classList.add("active");
      btnSettings.style.transform = "rotate(90deg)";
      // The sheet replaces the player body, so the window stays compact (~330 px)
      await morphExpanded(() => {
        liquidCard.classList.add("settings-open");
        settingsPanel.classList.remove("hidden");
      });
      updateSettingsTabIndicator();
      updateSettingsState();
    } else {
      // Closing without saving discards every edit (theme/opacity previews included)
      if (isConfigDirty()) await loadConfigUI();
      btnSettings.classList.remove("active");
      btnSettings.style.transform = "";
      await morphExpanded(() => {
        liquidCard.classList.remove("settings-open");
        settingsPanel.classList.add("hidden");
        applyLyricsPanelDom();
      });
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

// Shown under the bind when the backend rejects the hotkey (already registered elsewhere)
function clearBindError() {
  bindError.textContent = "";
  bindError.classList.add("hidden");
}

function showBindError(message) {
  bindError.textContent = message;
  bindError.classList.remove("hidden");
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

// =========================================================
// Settings sheet: tabs, segmented controls, dependencies, unsaved state
// =========================================================

const settingsTabs = document.getElementById("settingsTabs");
const settingsHint = document.getElementById("settingsHint");
const SETTINGS_TAB_ORDER = ["general", "island", "look", "system", "notify"];
let settingsTab = "general";

// Slide the pill under the active tab (measured, so labels can have any width)
function updateSettingsTabIndicator() {
  const active = settingsTabs.querySelector(".settings-tab.active");
  if (!active) return;
  settingsTabs.style.setProperty("--tab-x", `${active.offsetLeft - 2}px`);
  settingsTabs.style.setProperty("--tab-w", `${active.offsetWidth}px`);
}

function showSettingsTab(tab) {
  if (!SETTINGS_TAB_ORDER.includes(tab)) return;
  // Pages slide in from the side of the tab you came from
  const dir = SETTINGS_TAB_ORDER.indexOf(tab) >= SETTINGS_TAB_ORDER.indexOf(settingsTab) ? 1 : -1;
  settingsTab = tab;
  settingsTabs.querySelectorAll(".settings-tab").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.tab === tab);
    btn.setAttribute("aria-selected", btn.dataset.tab === tab ? "true" : "false");
  });
  settingsPanel.querySelectorAll(".settings-page").forEach((page) => {
    const on = page.dataset.page === tab;
    page.style.setProperty("--page-from", `${dir * 10}px`);
    page.classList.toggle("active", on);
    if (on) page.scrollTop = 0;
  });
  settingsPanel.querySelector(".settings-pages").scrollTop = 0;
  updateSettingsTabIndicator();
  if (tab === "notify") void refreshNotifAccess();
}

settingsTabs.querySelectorAll(".settings-tab").forEach((btn) => {
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    showSettingsTab(btn.dataset.tab);
  });
});

// Segmented control with a select-like `.value`, so the config code reads it unchanged
function makeSegmented(el) {
  const buttons = Array.from(el.querySelectorAll("button"));
  let current = buttons.length ? buttons[0].dataset.value : "";
  const paint = () => buttons.forEach((b) => b.classList.toggle("active", b.dataset.value === current));
  Object.defineProperty(el, "value", {
    get: () => current,
    set: (v) => {
      const match = buttons.find((b) => Number(b.dataset.value) === Number(v));
      if (match) current = match.dataset.value;
      paint();
    },
  });
  buttons.forEach((b) =>
    b.addEventListener("click", (e) => {
      e.preventDefault();
      current = b.dataset.value;
      paint();
      el.dispatchEvent(new Event("change", { bubbles: true }));
    })
  );
  paint();
}

[volumeStepSelect, expandDelaySelect, statsIntervalSelect, notifDurationSelect].forEach(makeSegmented);

// "Teams, Outlook ,, teams" -> ["Teams", "Outlook"]
function parseIgnoreList(text) {
  const out = [];
  for (const part of String(text || "").split(",")) {
    const name = part.trim();
    if (name && !out.some((x) => x.toLowerCase() === name.toLowerCase())) out.push(name);
  }
  return out.slice(0, 30);
}

const NOTIF_ACCESS_TEXT = {
  allowed: "Acceso permitido · copia silenciosa arriba a la derecha",
  denied: "Sin acceso: actívalo en Configuración › Privacidad › Notificaciones",
  unavailable: "Este Windows no permite leer las notificaciones",
  unspecified: "Copia silenciosa arriba a la derecha, sin sonido ni foco",
};

// The poller only learns the permission while the feature is on
async function refreshNotifAccess() {
  try {
    const status = await invoke("notify_access");
    notifAccess.textContent = NOTIF_ACCESS_TEXT[status] || NOTIF_ACCESS_TEXT.unspecified;
    notifAccess.classList.toggle("warn", status === "denied" || status === "unavailable");
  } catch {
    notifAccess.textContent = NOTIF_ACCESS_TEXT.unspecified;
  }
}

btnNotifTest.addEventListener("click", (e) => {
  e.preventDefault();
  invoke("notify_test").catch((err) => console.error("Notify test error:", err));
});

// Rows that only matter when a master switch is on (or off) fade out and lock
function updateSettingsDependencies() {
  const island = chkIslandEnabled.checked;
  const stats = chkStatsEnabled.checked;
  const notify = chkNotifEnabled.checked;
  settingsPanel.querySelectorAll("[data-dep]").forEach((row) => {
    const dep = row.dataset.dep;
    const enabled =
      dep === "island" ? island : dep === "no-island" ? !island : dep === "notify" ? notify : stats;
    row.classList.toggle("disabled", !enabled);
    row.querySelectorAll("input, button, select").forEach((el) => {
      el.disabled = !enabled;
    });
  });
}

// Comparable snapshot: list order and number formatting must not count as a change
function configFingerprint(cfg) {
  if (!cfg) return "";
  const keys = Object.keys(collectConfig()).sort();
  const out = {};
  for (const k of keys) {
    let v = cfg[k];
    if (k === "stats_items" && Array.isArray(v)) v = [...v].sort();
    if (k === "hotkey_modifiers" && Array.isArray(v)) v = v.map((m) => m.toLowerCase()).sort();
    if (typeof v === "number") v = Math.round(v * 1000) / 1000;
    out[k] = v;
  }
  return JSON.stringify(out);
}

function isConfigDirty() {
  return !!appConfig && configFingerprint(collectConfig()) !== configFingerprint(appConfig);
}

function updateSettingsState() {
  updateSettingsDependencies();
  if (btnSaveConfig.classList.contains("saved")) return; // keep the confirmation visible
  const dirty = isConfigDirty();
  btnSaveConfig.disabled = !dirty;
  settingsHint.textContent = dirty ? "Cambios sin guardar" : "Sin cambios";
  settingsHint.classList.toggle("dirty", dirty);
}

// Any interaction inside the sheet may change the draft (after the control's own handler ran)
["input", "change", "click"].forEach((type) =>
  settingsPanel.addEventListener(type, () => setTimeout(updateSettingsState, 0))
);

// Theme selector: immediate preview, persisted on Save
themeSwatches.forEach((btn) => {
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    themePreview = btn.dataset.theme;
    applyTheme(themePreview);
  });
});

cardOpacityRange.addEventListener("input", () => applyOpacity(cardOpacityRange.value));

// Auto theme follows the system scheme live
if (window.matchMedia) {
  const lightMq = window.matchMedia("(prefers-color-scheme: light)");
  if (lightMq.addEventListener) {
    lightMq.addEventListener("change", () => {
      if (themePreview === "auto") applyTheme("auto");
    });
  }
}

// Enable/disable dragging: the custom JS drag (start_drag) is the only mechanism
function applyDraggable(enabled) {
  liquidCard.classList.toggle("no-drag", !enabled);
}

// Hover-reveal toggle lives as a header icon, left of the settings gear
function updateHoverRevealIcon() {
  const on = !!(appConfig && appConfig.hover_reveal);
  btnHoverReveal.classList.toggle("active", on);
}

btnHoverReveal.addEventListener("click", async (e) => {
  e.stopPropagation();
  if (!appConfig) return;
  appConfig.hover_reveal = !appConfig.hover_reveal;
  updateHoverRevealIcon();
  try {
    await invoke("update_app_config", { newConfig: appConfig });
  } catch (err) {
    console.error("Save hover reveal error:", err);
  }
});

// Modifier pills + key from a config (also used to revert a rejected bind)
function applyHotkeyUI(cfg) {
  const mods = Array.isArray(cfg.hotkey_modifiers) ? cfg.hotkey_modifiers : [];
  modBtns.forEach((btn) => {
    const isMod = mods.some((m) => m.toLowerCase() === btn.dataset.mod.toLowerCase());
    btn.classList.toggle("active", isMod);
  });
  keySelect.value = cfg.hotkey_key || "Space";
  updateBindBadge();
}

// Load Configuration into UI
async function loadConfigUI() {
  try {
    appConfig = await invoke("get_app_config");

    // Theme and opacity (preview state starts in sync with the saved config)
    themeChoice = THEME_IDS.includes(appConfig.theme) ? appConfig.theme : "liquid";
    themePreview = themeChoice;
    applyTheme(themeChoice);
    chkAccentFromArt.checked = appConfig.accent_from_art !== false;
    applyOpacity(appConfig.card_opacity ?? 0.74);

    applyHotkeyUI(appConfig);

    // Wheel volume step: snap legacy/odd values to the closest preset
    const step = Number(appConfig.volume_step);
    const steps = [0.01, 0.02, 0.05, 0.1];
    const closest = steps.reduce(
      (best, s) => (Math.abs(s - step) < Math.abs(best - step) ? s : best),
      steps[0]
    );
    volumeStepSelect.value = String(closest);
    clearBindError();

    // Toggles
    chkAutoHide.checked = !!appConfig.auto_hide_on_blur;
    chkAmbientGlow.checked = appConfig.ambient_glow !== false;
    chkAppVolume.checked = appConfig.app_volume !== false;
    chkDraggable.checked = appConfig.draggable !== false;
    applyDraggable(appConfig.draggable !== false);
    chkRespectFullscreen.checked = appConfig.respect_fullscreen !== false;
    chkLyrics.checked = !!appConfig.lyrics_enabled;
    chkIslandEnabled.checked = appConfig.island_enabled !== false;
    chkIslandAlwaysVisible.checked = appConfig.island_always_visible !== false;
    chkIslandActivities.checked = appConfig.island_activities !== false;
    const delay = String(Number(appConfig.hover_expand_delay_ms));
    const delays = ["0", "150", "250", "500", "1000"];
    expandDelaySelect.value = delays.includes(delay) ? delay : "250";

    chkStatsEnabled.checked = !!appConfig.stats_enabled;
    const statInt = String(Number(appConfig.stats_interval_ms));
    statsIntervalSelect.value = ["500", "1000", "2000", "5000"].includes(statInt)
      ? statInt
      : "1000";
    const statItems = Array.isArray(appConfig.stats_items) ? appConfig.stats_items : [];
    chkStatCpu.checked = statItems.includes("cpu");
    chkStatGpu.checked = statItems.includes("gpu");
    chkStatRam.checked = statItems.includes("ram");
    chkStatVram.checked = statItems.includes("vram");
    chkStatNet.checked = statItems.includes("net");
    chkStatDisk.checked = statItems.includes("disk");
    chkStatGpuTemp.checked = statItems.includes("gpu_temp");
    chkStatsInIsland.checked = !!appConfig.stats_in_island;
    chkStatsAlerts.checked = !!appConfig.stats_alerts;
    chkNotifEnabled.checked = !!appConfig.notif_enabled;
    chkNotifShowBody.checked = appConfig.notif_show_body !== false;
    notifDurationSelect.value = String(Number(appConfig.notif_duration_ms) || 6000);
    notifIgnoreInput.value = (Array.isArray(appConfig.notif_ignore) ? appConfig.notif_ignore : []).join(", ");
    updateHoverRevealIcon();
    updateSettingsState();

    if (!chkAmbientGlow.checked) {
      disableAmbientGlow();
    }
  } catch (err) {
    console.error("Error loading config:", err);
  }
}

// The config the sheet currently describes (saved or not)
function collectConfig() {
  return {
    hotkey_modifiers: getSelectedModifiers(),
    hotkey_key: keySelect.value,
    // The app is always the Dynamic Island (top-center); legacy "custom" is normalized away
    position_preset: "top-center",
    custom_x: appConfig ? appConfig.custom_x : 100,
    custom_y: appConfig ? appConfig.custom_y : 100,
    auto_hide_on_blur: chkAutoHide.checked,
    ambient_glow: chkAmbientGlow.checked,
    volume_step: Number(volumeStepSelect.value) || 0.05,
    hover_reveal: !!(appConfig && appConfig.hover_reveal),
    app_volume: chkAppVolume.checked,
    draggable: chkDraggable.checked,
    respect_fullscreen: chkRespectFullscreen.checked,
    lyrics_enabled: chkLyrics.checked,
    island_enabled: chkIslandEnabled.checked,
    island_always_visible: chkIslandAlwaysVisible.checked,
    island_activities: chkIslandActivities.checked,
    hover_expand_delay_ms: Number(expandDelaySelect.value) || 0,
    stats_enabled: chkStatsEnabled.checked,
    stats_interval_ms: Number(statsIntervalSelect.value) || 1000,
    stats_items: [
      chkStatCpu.checked ? "cpu" : null,
      chkStatGpu.checked ? "gpu" : null,
      chkStatRam.checked ? "ram" : null,
      chkStatVram.checked ? "vram" : null,
      chkStatNet.checked ? "net" : null,
      chkStatDisk.checked ? "disk" : null,
      chkStatGpuTemp.checked ? "gpu_temp" : null,
    ].filter(Boolean),
    stats_in_island: chkStatsInIsland.checked,
    stats_alerts: chkStatsAlerts.checked,
    notif_enabled: chkNotifEnabled.checked,
    notif_show_body: chkNotifShowBody.checked,
    notif_duration_ms: Number(notifDurationSelect.value) || 6000,
    notif_ignore: parseIgnoreList(notifIgnoreInput.value),
    theme: themePreview,
    accent_from_art: chkAccentFromArt.checked,
    card_opacity: Number(cardOpacityRange.value) || 0.74,
  };
}

// Save Configuration
btnSaveConfig.addEventListener("click", async () => {
  if (btnSaveConfig.disabled) return;
  const newConfig = collectConfig();

  try {
    await invoke("update_app_config", { newConfig });
    appConfig = newConfig;
    configDirty = false;
    clearBindError();
    themeChoice = newConfig.theme;
    themePreview = themeChoice;
    applyTheme(themeChoice);
    applyOpacity(newConfig.card_opacity);
    applyDraggable(chkDraggable.checked);
    // Ambient glow toggle takes effect immediately
    if (chkAmbientGlow.checked) {
      if (lastThumbnailBase64) setAmbientGlow(lastThumbnailBase64);
    } else {
      disableAmbientGlow();
    }
    // Volume target may have switched between app/system
    void syncMasterVolume();
    // Lyrics toggle changed: re-evaluate for the current track
    lyricsTrackKey = "";
    lyricsRequestKey = "";
    void refreshMediaState();

    // Stats visibility/targets may have changed
    applyStatsConfig();
    if (!chkStatsInIsland.checked) islandStatsAlt = false;
    if (lastMediaState) updateIslandCompact(lastMediaState);

    // The poller re-reads the permission once it runs with the new config
    if (newConfig.notif_enabled) setTimeout(() => void refreshNotifAccess(), 2000);

    // Button feedback; the sheet stays open so you can keep tweaking other tabs
    btnSaveConfig.innerHTML = "<span>✓ Guardado</span>";
    btnSaveConfig.classList.add("saved");
    settingsHint.textContent = "Preferencias guardadas";
    settingsHint.classList.remove("dirty");

    setTimeout(() => {
      btnSaveConfig.innerHTML = "<span>Guardar</span>";
      btnSaveConfig.classList.remove("saved");
      updateSettingsState();
    }, 1200);
  } catch (err) {
    console.error("Save config error:", err);
    // The backend rejected the change (usually the hotkey is taken): revert only the bind,
    // keep the rest of the draft so nothing else is lost
    if (appConfig) applyHotkeyUI(appConfig);
    showSettingsTab("general");
    showBindError(String(err));
    updateSettingsState();
  }
});

// Escape key to hide (in island mode it collapses instead of hiding the window)
window.addEventListener("keydown", async (e) => {
  if (e.key === "Escape") {
    if (isSourcePopoverOpen) {
      toggleSourcePopover(false);
    } else if (isSettingsOpen) {
      toggleSettings(false);
    } else if (appConfig && appConfig.island_enabled) {
      if (islandMode === "expanded") {
        invoke("request_island_mode", { mode: "compact" }).catch(() => {});
      }
    } else {
      await invoke("hide_window");
    }
  }
});

// Native window dragging via Tauri start_drag command
document.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return; // Only primary mouse button

  // Compact/activity: a click expands the island (dragging happens in expanded mode)
  if (islandMode === "compact" || islandMode === "activity") {
    if (appConfig && appConfig.island_enabled) {
      invoke("request_island_mode", { mode: "expanded" }).catch((err) => {
        console.warn("expand error:", err);
      });
    }
    return;
  }

  // Dragging disabled by the user setting
  if (appConfig && appConfig.draggable === false) return;

  // Never initiate drag if interacting with buttons, inputs, sliders, popovers, etc.
  if (e.target.closest("button, input, select, .scrubber, .vol-bar, .source-item, .mod-btn, .source-popover, .lyrics-scroll")) {
    return;
  }

  // Allow dragging on header, metadata wrapper, background card, island
  if (
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
      syncLyricsToPosition();
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

  // Apply the island state before the first content paint
  try {
    const info = await invoke("get_island_mode");
    applyIslandMode(info.mode);
  } catch (err) {
    console.warn("Island mode unavailable:", err);
    applyIslandMode("expanded");
  }

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

  // Island state changes (compact / activity / expanded / hidden)
  await listen("island-mode", (event) => {
    const payload = event.payload || {};
    applyIslandMode(payload.mode);
  });

  // System stats from the backend sampler
  await listen("system-stats", (event) => {
    renderStats(event.payload);
  });

  // Compact island: alternate between the title and a stats summary every 4 s
  setInterval(() => {
    if (islandMode !== "compact" || !appConfig || !appConfig.stats_in_island || !lastStats) {
      return;
    }
    islandStatsAlt = !islandStatsAlt;
    if (lastMediaState) updateIslandCompact(lastMediaState);
  }, 4000);

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
  document.documentElement.addEventListener("mouseenter", () => liquidCard.classList.remove("island-out"));

  // Stats chips/visibility follow the saved config from the first frame
  applyStatsConfig();
}

window.addEventListener("DOMContentLoaded", init);
