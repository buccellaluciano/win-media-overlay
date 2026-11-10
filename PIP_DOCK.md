# Video acoplado (PiP del navegador) — plan de trabajo

Estado: **en curso**. Solo están hechas las opciones de config (commit `60f2e41`, escrito sin compilar).
Leé también `AGENTS.md`: sus reglas (sin descargas sin preguntar, `--offline`, APIs prohibidas) aplican acá.

## Objetivo

El usuario arrastra la ventana *Picture in Picture* de su navegador (Chrome, Edge, Brave, Firefox) y la
suelta sobre la tarjeta de WMO. El video queda **acoplado**: WMO mueve y redimensiona esa ventana real
para que encaje en un hueco de la tarjeta y la sigue cuando la app se mueve.

Decisiones ya tomadas con el usuario:

- **Opción B: acoplar la ventana real**, no capturarla. Chrome sigue dibujando el video, así que el coste
  es cero, funciona con DRM (Netflix, etc.) y los controles propios del PiP siguen andando.
  - Descartado `SetParent` entre procesos: es frágil (DPI, foco, render) y es manipulación ajena.
  - Descartada la captura con Windows.Graphics.Capture: el DRM se ve negro y es cara en la WebView. Quizá
    más adelante como modo "integrado".
- **Dos diseños** (`pip_layout`), elegibles en Ajustes:
  - `"cinema"` (por defecto): el video 16:9 ocupa todo el ancho donde estaban carátula y título.
    Debajo queda una barra mínima con título, progreso, controles y volumen.
  - `"art"`: el video va en el lugar de la carátula, a **260×146**, que es el tamaño mínimo que acepta el
    PiP de Chrome. Pensado para videos musicales (Spotify web, YouTube Music…).
- **Con la isla colapsada** (`pip_island`), elegible en Ajustes:
  - `"hide"` (por defecto): se oculta el PiP.
  - `"float"`: queda flotando chico, centrado debajo de la isla.

## Lo que ya existe

- `src/config.rs`: `pip_dock_enabled` (true), `pip_layout` ("cinema" | "art") y `pip_island`
  ("hide" | "float"), con `PIP_LAYOUTS`, `PIP_ISLAND`, defaults y `normalize()`.
- **Falta** reflejarlos en `ui/app.js`, en `collectConfig()` y en `loadConfigUI()`. Mientras falten, cada
  "Guardar" los devuelve a los valores por defecto.

## Contexto del código que hay que conocer

- La ventana principal mide **siempre 480×460** (`WINDOW_W` y `WINDOW_H` en `src/main.rs`). Solo la
  tarjeta cambia de tamaño, en CSS. Fuera de la tarjeta hace click-through: el frontend informa el
  rectángulo con `set_hit_rect` y el hilo de hover de 50 ms alterna `set_ignore_cursor_events`.
  **No redimensiones la ventana nativa**: WebView2 muestra un frame viejo y la tarjeta "salta" de lado.
- Modos de la isla: `IslandMode::{Hidden, Compact, Activity, Expanded}`. Todos los cambios pasan por
  `set_island_mode_internal`. El frontend recibe el evento `island-mode`.
- La ventana se mueve con el drag propio (`start_drag`). Eso dispara `WindowEvent::Moved` en el
  `on_window_event` de `main.rs`, que hoy no hace nada.
- Coordenadas: `win.inner_position()` está en px físicos y la UI en px CSS × `scale_factor()`. Para
  convertir, mirá `cursor_in_card()`.
- Reproductor (`ui/index.html`): `#playerBody` contiene `#artContainer` (74×74) y `.content-container`,
  que a su vez contiene metadatos, línea de tiempo y `.actions-row`. Debajo van `#statsStrip` y el panel
  de letras.

## Pasos

### 0. Toolchain
- [ ] Instalar Rust (rustup). La primera compilación es **sin** `--offline` para bajar los crates que fija
  `Cargo.lock`; no se agregan dependencias. Desde ahí, `cargo build --release --offline`.
- [ ] Verificar que compila el commit `60f2e41` (las opciones de config) antes de seguir.

### 1. Backend: `src/pip.rs` (nuevo)
APIs Win32 permitidas: no son hooks, no abren procesos y no inyectan nada. Verificá cada firma en
`~/.cargo/registry/src/*/windows-0.61.3/src/Windows/Win32/UI/WindowsAndMessaging/mod.rs` antes de usarla.
Probablemente no haga falta ningún feature nuevo en `Cargo.toml`, porque `Win32_UI_WindowsAndMessaging`
ya está.

- [ ] **Detección de candidatos** (solo si `pip_dock_enabled`, la ventana principal está visible y el
  modo es `Expanded`):
  - `EnumWindows` (enumera ventanas, **no procesos**). Filtrar con `IsWindowVisible`, `GWL_EXSTYLE` con
    `WS_EX_TOPMOST`, `GetClassNameW` en `Chrome_WidgetWin_1` / `MozillaDialogClass` /
    `MozillaWindowClass`, y `GetWindowTextW` con un título de PiP. Comparar en minúsculas contra
    "picture in picture", "picture-in-picture", "imagen en imagen", "imagen con imagen" y
    "pantalla en pantalla".
  - **Confirmar los títulos reales** con el navegador del usuario en español; pueden variar.
  - Excluir las ventanas propias: `GetWindowThreadProcessId` da el PID sin abrir el proceso, y debe ser
    distinto de `std::process::id()`.
- [ ] **Arrastre y soltar**:
  - Se considera que el usuario está arrastrando si `GetGUIThreadInfo(tid)` tiene el flag
    `GUI_INMOVESIZE`, o si el rect cambió desde el último tick. Tick cada ~100 ms.
  - Mientras se arrastra y `GetCursorPos` está sobre la tarjeta, emitir `pip-drop-hover {over:true}`.
  - Soltar: el candidato estaba moviéndose, lleva ≥2 ticks quieto y el cursor está en la tarjeta. Ahí se
    acopla: guardar el `RECT` original y emitir `pip-docked {docked:true}`.
  - **Nada** de `GetAsyncKeyState`, hooks de mouse ni `SetWinEventHook`.
- [ ] **Estado**:
  - `static` con `Mutex` que guarda el `hwnd` como `isize` (HWND no es Send), el rect original, el slot
    actual y el flag de arrastre.
- [ ] **`sync(app)`**, que aplica la posición. Llamarla desde el tick, desde `WindowEvent::Moved`, desde
  el comando del slot y desde cada cambio de modo:
  - Si `!IsWindow(hwnd)`, desacoplar y emitir `pip-docked {docked:false}`.
  - Modo `Expanded`, con ventana visible y slot conocido: `SetWindowPos(hwnd, HWND_TOPMOST, rect del slot
    en pantalla, SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS)`. Mostrarlo con `ShowWindowAsync(SW_SHOWNOACTIVATE)`
    si estaba oculto.
    - Solo llamar si el rect cambió (comparar con `GetWindowRect`).
    - Mantenerlo **encima** de la ventana principal: recorrer `GetWindow(hwnd, GW_HWNDPREV)`. Si aparece
      la principal, re-subir el PiP con `SWP_NOMOVE | SWP_NOSIZE`.
  - Modo `Compact` o `Activity`:
    - Con `"hide"`: `ShowWindowAsync(SW_HIDE)`.
    - Con `"float"`: ~320×180 lógicos, centrado debajo de la isla.
  - Ventana principal oculta: con `"hide"` se oculta; con `"float"` se queda donde está.
  - Usar siempre las variantes **async** (`SWP_ASYNCWINDOWPOS`, `ShowWindowAsync`) para no bloquearse si
    el navegador está colgado.
- [ ] **Desacoplar**:
  - Si el usuario arrastra el PiP acoplado (`GUI_INMOVESIZE`), dejar de forzar la posición. Al soltar:
    - si está sobre la tarjeta, vuelve al slot;
    - si no, se desacopla y recupera su **tamaño original** donde quedó.
  - Comando `pip_undock`: restaura el rect original completo y lo muestra.
  - `release()`: restaura y muestra. Llamarla al desactivar la opción, al cambiar a otro PiP y en "Salir"
    (bandeja, `"quit"`), antes de `app.exit(0)`.
- [ ] **Comandos Tauri**:
  - `pip_set_slot(x, y, width, height)`: px CSS en la ventana; `width == 0` significa sin slot. Validar
    `is_finite` y clamp como `set_hit_rect`.
  - `pip_undock()`.
  - `pip_status() -> bool`, para el arranque del frontend.
  - Registrarlos en `generate_handler!` y lanzar `pip::spawn(app)` en `setup`.
- [ ] **Coste**: si la opción está apagada o no hay ventana visible en `Expanded` sin PiP acoplado, el
  hilo duerme (500 ms o canal `wake()` como `sysstats`). `update_app_config` y
  `set_island_mode_internal` deben llamar a `pip::wake()` / `pip::sync()`.

### 2. Frontend
- [ ] **Estado**:
  - `pipDocked` se actualiza con `pip-docked` y `pip_status()`.
  - Clase `pip-docked` + `pip-cinema` o `pip-art` en `.liquid-card`.
- [ ] **Hueco** (`<div class="pip-slot" id="pipSlot">`, oculto si no hay video acoplado):
  - `cinema`: `#playerBody` pasa a columna, el slot va a ancho completo con `aspect-ratio: 16/9` (~438×246)
    y se ocultan carátula y metadatos grandes; la línea de tiempo y `.actions-row` quedan debajo.
    Tiene que entrar en 460 px de alto (≈ 40 de encabezado + 246 + ~100 de controles). Si no entra con
    letras o estadísticas visibles, ocultar las letras mientras haya video.
  - `art`: `#artContainer` mide 260×146. Grid de dos columnas: video y metadatos arriba, y la línea de
    tiempo y las acciones a todo el ancho debajo (`display: contents` en `.content-container`).
  - Fondo del slot: `var(--surface-sunken)` con el radio del tema, como marco del video.
- [ ] **Informar el slot**:
  - `reportPipSlot()` hace como `reportHitRect`: `ResizeObserver` + dedupe y `invoke("pip_set_slot", …)`.
  - Mandar `width 0` cuando no está visible (modo isla o ajustes abiertos).
  - Llamarlo después del morph (`morphCard` / `morphExpanded` limpian con un timer: reportar ahí).
- [ ] **Zona para soltar**: con `pip-drop-hover`, borde/resplandor del acento sobre la tarjeta y el texto
  "Soltá para acoplar el video".
- [ ] **Botón en el encabezado**, visible solo con video acoplado, para desacoplar. Mismo estilo que
  `btnQuickDrag`.
- [ ] **Ajustes**: una pestaña nueva "Video", o una sección en General si 6 pestañas no entran. Contiene:
  - interruptor "Acoplar video de los navegadores" (`pip_dock_enabled`);
  - segmentado "Diseño: Modo cine / En lugar de la carátula" (`pip_layout`);
  - segmentado "Con la isla: Ocultar / Flotante" (`pip_island`), con `data-dep`.
  - Agregarlos a `collectConfig()`, `loadConfigUI()` y `configFingerprint`.
- Texto de la UI en español; nunca `innerHTML` con títulos de ventana.

### 3. Verificación
- [ ] `cargo build --release --offline` y `cargo clippy --offline` sin warnings nuevos.
- [ ] El grep de APIs prohibidas de `AGENTS.md` no devuelve nada.
- [ ] Prueba manual con Chrome o Edge (YouTube) y Firefox:
  - acoplar soltando, en los dos diseños;
  - mover la app y comprobar que el video la sigue;
  - colapsar a la isla con "hide" y con "float";
  - desacoplar arrastrando y con el botón;
  - cerrar el PiP desde el navegador;
  - "Salir" desde la bandeja y comprobar que el PiP vuelve a su lugar.
- [ ] Revisar que con la opción apagada, o sin PiP, el consumo de CPU no cambie (Administrador de tareas).
- [ ] Actualizar `README.md` (función y lista de seguridad: `EnumWindows`, `SetWindowPos` sobre la ventana
  PiP, sin hooks) y `CHANGELOG.md`.
- [ ] Un commit por bloque (backend, frontend), con el trailer `Co-Authored-By` que use el repo.

## Riesgos conocidos
- **Títulos localizados**: si no detecta el PiP, revisar el título real de la ventana (Spy++ o un
  `EnumWindows` de prueba) y agregarlo a la lista.
- **`SW_HIDE` sobre la ventana de Chrome**: confirmar que no pausa ni cierra el video. Si da problemas,
  probar sacándolo fuera de pantalla.
- **Si WMO crashea con el PiP oculto**, queda invisible hasta que se cierre desde la pestaña.
  Considerarlo en el diseño (por ejemplo, no ocultar y usar "float" si se detecta el problema).
- **Spotify de escritorio** no usa el PiP del navegador; su mini reproductor queda fuera de alcance por
  ahora.
