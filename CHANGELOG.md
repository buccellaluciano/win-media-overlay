# Changelog

Todas las fechas en formato ISO. Una entrada por fase del encargo original.

## [0.2.0] — 2026-10-27

### Fase 1 — Corrección de bugs (`9a0b732`)

- Eliminada la entrada sintética de teclado (`keybd_event`) y el comando `adjust_volume` muerto.
- Volumen por app sin `OpenProcess`: matching por identificador de sesión WASAPI (ruta del exe) con caché `pid → stem` de 5 s.
- `fullscreen_active` compartido en `AppState`: guardar ajustes ya no re-asegura *always on top* sobre un juego, y el atajo muestra sin foco (`SW_SHOWNOACTIVATE`).
- Firefox reconocido por su AUMID (`308046B0AF4A39CB`) + fallback a la única sesión WASAPI activa.
- `source_name` calculado en Rust; `formatSourceApp` eliminado del JS y `edge` ya no matchea de más.
- Rueda de volumen: desmutea de verdad en el backend y usa `volume_step` (select 1/2/5/10 %).
- Config corrupta → `config.json.bak-<timestamp>` antes de guardar defaults; valores validados/clamp.
- `panic.log` en `%APPDATA%\WMO\logs\` con append, timestamp y tope de 256 KB.
- Atajo global con `GetMessageW` bloqueante, `PostThreadMessageW`, resultado de registro devuelto a la UI y `Drop` que desregistra en su hilo.
- Un único mecanismo de arrastre; hover en el monitor del cursor (`MonitorFromPoint` + `GetDpiForMonitor`); manager GSMTC recreado si falla; MIME real de la carátula; timeout y caché acotada de letras; CSS muerto eliminado.
- Higiene de binario: `lto`, `codegen-units = 1`, `opt-level = "s"`, `strip`, metadatos de bundle y sección de firma en README.

### Fase 2 — Isla dinámica real (`0a16ac0`, `d089b40`)

- Estados `Hidden / Compact / Activity / Expanded` con dueño en el backend (`set_island_mode`): tamaño nativo, posición top-center y evento `island-mode`.
- Morph sin parpadeo: al crecer se agranda la ventana y luego anima el CSS; al encoger anima primero y reduce la ventana 220 ms después. La ventana mide exactamente lo visible.
- Altura animada por JS (36/56 ↔ altura natural) y crossfade entre el reproductor y las vistas compacta/activity.
- Hover con retardo configurable y colapso a los 450 ms; ajustes/popover fijan la isla.
- Actividades en cola (máx. 3, sin duplicados consecutivos, 2,5 s) para pista, fuente y volumen.
- Clic a través opcional en compacta; atajo que alterna expandida ↔ compacta/oculta; fullscreen exclusivo oculta y restaura el modo previo.
- Config nueva: `island_enabled`, `island_always_visible`, `island_activities`, `island_click_through_compact`, `hover_expand_delay_ms`.

### Fase 3 — Estadísticas del sistema (`a8dfee7`)

- Nuevo `src/sysstats.rs` con un único hilo muestreador, creado de forma perezosa y **bloqueado** cuando las stats están desactivadas o la ventana oculta.
- CPU (`GetSystemTimes`), RAM (`GlobalMemoryStatusEx`), GPU/VRAM/disco (PDH), red (`GetIfTable2`) y temperatura NVIDIA opcional (`nvml.dll` solo si ya existe).
- Evento `system-stats`; primera lectura descartada; query PDH y NVML liberados al desactivar.
- Chips con barra de color, actualización solo de nodos cambiados, resumen opcional en la isla compacta y alertas de CPU/GPU/RAM con histéresis y límite temporal.
- Config nueva: `stats_enabled`, `stats_interval_ms`, `stats_items`, `stats_in_island`, `stats_alerts`.

### Fase 4 — Temas (`f3f4dfd`)

- `style.css` tokenizado por completo: colores, radios, blurs y sombras solo en bloques de variables.
- 9 temas: `liquid`, `frost`, `oled`, `mica`, `neon`, `sunset`, `minimal`, `contrast` y `auto` (sigue el modo claro/oscuro del sistema en vivo).
- Selector visual de swatches con previsualización inmediata, reversión al cerrar sin guardar y persistencia al confirmar.
- Acento desde la carátula (desactivable); algunos temas lo fijan por diseño. Slider de opacidad con mínimos por tema.
- Config nueva: `theme`, `accent_from_art`, `card_opacity`.
- Botón Guardar con confirmación por clase CSS y `--success` (sin hex inline).

### Fase 5 — Heurística de selección de fuente (`5bd300b`)

- Puntuación por sesión GSMTC: +100 reproduciendo, +40 sesión mostrada, +20 título/duración, +10 sesión actual de Windows, −50 recién iniciada (<1,5 s), −30 duración <8 s; empate a favor de la mostrada.
- Metadatos por sesión cacheados 2 s y seguimiento de inicio de reproducción por AUMID.
- Pausa ≠ cambio: cambio tras el hold de 1,5 s cuando otra fuente suena; si nadie suena, se queda la mostrada.
- Selección manual estable con `id + título` y desempate por título antes que por índice en AUMIDs duplicados.
- `current_source_app_id` reutiliza el AUMID mostrado; `select_audio_source` pasa a `async`.
- Tests del scoring añadidos.

### Fase 6 — Documentación

- README reescrito: isla dinámica, estadísticas, temas, seguridad/anticheat, limitación de fullscreen exclusivo y estructura actualizada.
- `CHANGELOG.md` añadido.
- Versión subida a **0.2.0** en `Cargo.toml` y `tauri.conf.json`.
