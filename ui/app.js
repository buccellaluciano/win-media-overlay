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
const chkIslandClickThrough = document.getElementById("chkIslandClickThrough");
const expandDelaySelect = document.getElementById("expandDelaySelect");

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
let currentWindowWidth = 480;
let currentWindowHeight = 188;
let islandMode = ""; // compact | activity | expanded | hidden, owned by the backend
let lastMediaState = null;
let lastIslandTrackKey = "";
let lastIslandSource = "";
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

// Measure the card and fit the window (same approach as settings; no magic heights)
function updateLyricsPanelHeight() {
  if (isSettingsOpen || isSourcePopoverOpen) return; // those own the window size
  if (!lyricsShouldShow()) {
    setWindowSize(480, 188);
    return;
  }
  const needed = Math.ceil(liquidCard.getBoundingClientRect().height) + 18;
  setWindowSize(480, Math.max(188, needed));
}

function setLyricsAvailable(available) {
  lyricsAvailable = !!available;
  if (!lyricsAvailable) {
    lyricsLines.innerHTML = "";
    lyricsLinesData = [];
    lyricsActiveIdx = -1;
  }
  updateLyricsChevron();
  applyLyricsPanelDom();
  updateLyricsPanelHeight();
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
  applyLyricsPanelDom();
  updateLyricsPanelHeight();
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

// Centralized window resize: skips redundant OS resizes (each one re-layouts + re-blurs the card)
async function setWindowSize(width, height) {
  // Compact/activity sizes are owned by the backend; content only resizes expanded
  if (islandMode && islandMode !== "expanded") return;
  if (width === currentWindowWidth && height === currentWindowHeight) return;
  currentWindowWidth = width;
  currentWindowHeight = height;
  try {
    await invoke("set_window_size", { width, height });
  } catch (err) {
    console.error("set_window_size error:", err);
  }
}

// Usable height on the window's own monitor, in physical px -> logical.
// ponytail: screen.availHeight in the WebView ignores which monitor/where it sits, so tail got cut.
async function maxWindowHeight() {
  try {
    const appWindow = window.__TAURI__.window.getCurrentWindow();
    const mon = await appWindow.currentMonitor();
    const sf = await appWindow.scaleFactor();
    if (mon && mon.size && sf > 0) {
      return Math.floor(mon.size.height / sf) - 24;
    }
  } catch (err) {
    // fall through to the browser fallback
  }
  return (window.screen && window.screen.availHeight) || 1080;
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
  const title = has ? state.title || "Sin reproducción" : "Sin reproducción";
  if (islandTitle.textContent !== title) {
    islandTitle.textContent = title;
    requestAnimationFrame(() => checkMarquee(islandTitle, islandTitleBox));
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
    to = Math.max(176, expandedView.scrollHeight + 2);
  } else {
    from = prevMode === "compact" ? 36 : 56;
    to = mode === "compact" ? 36 : 56;
  }
  if (!from || !to) return;

  // Same-task inline height pins the starting point; the rAF change animates it
  liquidCard.style.height = `${from}px`;
  void liquidCard.offsetHeight;
  requestAnimationFrame(() => {
    liquidCard.style.height = `${to}px`;
  });

  if (cardMorphTimer) clearTimeout(cardMorphTimer);
  cardMorphTimer = setTimeout(() => {
    cardMorphTimer = null;
    liquidCard.style.height = ""; // back to auto in expanded / CSS height in island modes
  }, 260);
}

// Single place where the frontend mirrors the backend island state
function applyIslandMode(mode, width, height) {
  if (!mode) return;
  const prevMode = islandMode;
  const collapseFrom =
    prevMode === "expanded" && (mode === "compact" || mode === "activity")
      ? liquidCard.getBoundingClientRect().height
      : 0;

  islandMode = mode;
  document.documentElement.dataset.mode = mode;
  if (Number.isFinite(width) && Number.isFinite(height) && mode !== "hidden") {
    currentWindowWidth = width;
    currentWindowHeight = height;
  }
  liquidCard.classList.remove("island-out");
  if (mode === "expanded") {
    clearActivities();
    applyLyricsPanelDom();
    updateLyricsPanelHeight();
  } else if (mode === "hidden") {
    clearActivities();
  } else if (mode === "compact") {
    clearActivities();
    if (lastMediaState) updateIslandCompact(lastMediaState);
  }

  morphCard(prevMode, mode, collapseFrom);
}

// Settings/popover must not collapse when the cursor leaves
function updateIslandPin() {
  const pinned = !!(isSettingsOpen || isSourcePopoverOpen);
  invoke("set_island_pinned", { pinned }).catch(() => {});
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
    lyricsPanel.classList.add("hidden");
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
      applyLyricsPanelDom();
      updateLyricsPanelHeight();
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
      // Lyrics panel yields to settings (they must not stack)
      lyricsPanel.classList.add("hidden");
      settingsPanel.classList.remove("hidden");
      btnSettings.style.transform = "rotate(60deg)";
      btnSettings.style.color = "var(--accent-color)";
      // Fit the window to the actual panel height (DPI/font independent, avoids clipping)
      settingsPanel.style.maxHeight = "none";
      const needed = Math.ceil(liquidCard.getBoundingClientRect().height) + 18;
      const max = await maxWindowHeight();
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
      btnSettings.style.transform = "rotate(0deg)";
      btnSettings.style.color = "";
      applyLyricsPanelDom();
      updateLyricsPanelHeight();
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
    chkIslandClickThrough.checked = !!appConfig.island_click_through_compact;
    const delay = String(Number(appConfig.hover_expand_delay_ms));
    const delays = ["0", "150", "250", "500", "1000"];
    expandDelaySelect.value = delays.includes(delay) ? delay : "250";
    updateHoverRevealIcon();

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
    island_click_through_compact: chkIslandClickThrough.checked,
    hover_expand_delay_ms: Number(expandDelaySelect.value) || 0,
  };

  try {
    await invoke("update_app_config", { newConfig });
    appConfig = newConfig;
    configDirty = false;
    clearBindError();
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
    // The backend rejected the change (usually the hotkey is taken): keep the saved bind
    await loadConfigUI();
    applyDraggable(chkDraggable.checked);
    showBindError(String(err));
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
    applyIslandMode(info.mode, info.width, info.height);
  } catch (err) {
    console.warn("Island mode unavailable:", err);
    applyIslandMode("expanded", 480, 188);
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
    applyIslandMode(payload.mode, payload.width, payload.height);
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
  document.documentElement.addEventListener("mouseenter", () => liquidCard.classList.remove("island-out"));
}

window.addEventListener("DOMContentLoaded", init);
