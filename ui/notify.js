// =========================================================
// WMO toasts: quiet copies of Windows notifications (top-right window).
// The window never takes focus; it only sizes itself to the stack.
// =========================================================

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const stack = document.getElementById("toastStack");
const MAX_TOASTS = 3;
const THEME_IDS = ["liquid", "frost", "oled", "mica", "neon", "sunset", "minimal", "contrast", "auto"];

let durationMs = 6000;
let themeChoice = "liquid";
let hovering = false;

// Same resolution as the player: "auto" follows the Windows light/dark setting
function applyTheme(cfg) {
  themeChoice = THEME_IDS.includes(cfg.theme) ? cfg.theme : "liquid";
  const light = window.matchMedia && window.matchMedia("(prefers-color-scheme: light)").matches;
  const resolved = themeChoice === "auto" ? (light ? "frost" : "liquid") : themeChoice;
  document.documentElement.dataset.theme = resolved;
  const opacity = Math.max(0.4, Math.min(1, Number(cfg.card_opacity) || 0.74));
  document.documentElement.style.setProperty("--card-opacity-user", String(opacity));
  durationMs = Math.max(3000, Math.min(15000, Number(cfg.notif_duration_ms) || 6000));
}

if (window.matchMedia) {
  const mq = window.matchMedia("(prefers-color-scheme: light)");
  if (mq.addEventListener) {
    mq.addEventListener("change", () => {
      if (themeChoice === "auto") applyTheme({ theme: "auto", card_opacity: currentOpacity() });
    });
  }
}

function currentOpacity() {
  return document.documentElement.style.getPropertyValue("--card-opacity-user") || 0.74;
}

// Window height follows the stack (top-anchored, fixed width: resizing can't shift content)
function reportLayout() {
  const height = stack.children.length ? Math.ceil(stack.getBoundingClientRect().height) : 0;
  invoke("notify_layout", { height }).catch(() => {});
}

// ---------------------------------------------------------
// Toast lifecycle: timer pauses while the pointer is over the stack
// ---------------------------------------------------------

function startTimer(el) {
  el._remaining = el._remaining ?? durationMs;
  el._startedAt = performance.now();
  el._timer = setTimeout(() => dismiss(el), el._remaining);
}

function pauseTimer(el) {
  if (!el._timer) return;
  clearTimeout(el._timer);
  el._timer = null;
  el._remaining = Math.max(400, el._remaining - (performance.now() - el._startedAt));
}

stack.addEventListener("mouseenter", () => {
  hovering = true;
  Array.from(stack.children).forEach(pauseTimer);
});

stack.addEventListener("mouseleave", () => {
  hovering = false;
  Array.from(stack.children).forEach((el) => {
    if (!el.classList.contains("leaving")) startTimer(el);
  });
});

function dismiss(el) {
  if (!el || el.classList.contains("leaving")) return;
  clearTimeout(el._timer);
  el._timer = null;
  el.classList.add("leaving");
  // After sliding out, collapse the row so the toasts below glide up
  setTimeout(() => {
    const h = el.getBoundingClientRect().height;
    const wrap = el;
    wrap.style.height = `${h}px`;
    wrap.classList.add("toast-collapse");
    void wrap.offsetHeight;
    wrap.style.height = "0px";
    wrap.style.marginTop = "-8px"; // swallow the flex gap too
    wrap.style.paddingTop = "0px";
    wrap.style.paddingBottom = "0px";
    setTimeout(() => {
      wrap.remove();
      reportLayout();
    }, 250);
  }, 240);
}

function initialOf(name) {
  const ch = String(name || "?").trim().charAt(0);
  return ch ? ch.toUpperCase() : "?";
}

function buildToast(n) {
  const el = document.createElement("div");
  el.className = "toast";
  el.style.setProperty("--toast-ms", `${durationMs}ms`);
  el.title = "Clic para descartar";

  let logo;
  if (n.logo) {
    logo = document.createElement("img");
    logo.src = n.logo;
    logo.alt = "";
  } else {
    logo = document.createElement("div");
    logo.textContent = initialOf(n.app);
  }
  logo.className = "toast-logo";

  const main = document.createElement("div");
  main.className = "toast-main";

  const head = document.createElement("div");
  head.className = "toast-head";
  const app = document.createElement("span");
  app.className = "toast-app";
  app.textContent = n.app || "Notificación"; // textContent: notification text is untrusted
  const time = document.createElement("span");
  time.textContent = "ahora";
  head.append(app, time);
  main.appendChild(head);

  if (n.title) {
    const title = document.createElement("div");
    title.className = "toast-title";
    title.textContent = n.title;
    main.appendChild(title);
  }
  if (n.body) {
    const body = document.createElement("div");
    body.className = "toast-body";
    body.textContent = n.body;
    main.appendChild(body);
  }

  const close = document.createElement("button");
  close.className = "toast-close";
  close.title = "Descartar";
  close.innerHTML =
    '<svg viewBox="0 0 24 24" width="10" height="10" stroke="currentColor" stroke-width="2.6" fill="none"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>';

  el.append(logo, main, close);
  el.addEventListener("click", () => dismiss(el));
  return el;
}

function showToast(n) {
  const el = buildToast(n);
  // Newest on top; anything past the limit leaves from the bottom
  stack.prepend(el);
  const live = Array.from(stack.children).filter((t) => !t.classList.contains("leaving"));
  live.slice(MAX_TOASTS).forEach(dismiss);
  if (!hovering) startTimer(el);
  reportLayout();
}

async function init() {
  try {
    applyTheme(await invoke("get_app_config"));
  } catch (err) {
    console.warn("Config unavailable:", err);
  }
  await listen("notification", (event) => {
    if (event.payload) showToast(event.payload);
  });
  await listen("config-changed", (event) => {
    if (event.payload) applyTheme(event.payload);
  });
  // Images (logos) can change the height after layout
  new ResizeObserver(reportLayout).observe(stack);
}

window.addEventListener("DOMContentLoaded", init);
