# Changelog

Todas las fechas en formato ISO. Una entrada por fase del encargo original.

## [1.1.0] — 2026-11-10

### Mejorado

- **Letras para videos:** los títulos de YouTube y similares ("Michael Jackson - Smooth Criminal (Official Video)" del canal "michaeljacksonVEVO") no coincidían con nada en LRCLIB. Nuevo `ui/lyrics-query.js`: quita etiquetas de subida (Official Video, Lyrics/Letra, HD/4K, Remastered, feat.…), separa "Artista - Canción" (también "Canción - Artista"), descarta lo que sigue a " | " y limpia el canal (VEVO, " - Topic", Official). Se busca primero lo que informa el reproductor y después hasta dos variantes limpias; entre los resultados se prefiere el de duración más cercana.

### Cambiado

- **Ícono nuevo:** monograma propio (W blanca sobre grafito con la píldora de la isla en azul), en 16–256 px y con trazo reforzado en los tamaños de la bandeja. Reemplaza la nube anterior y se quitó una foto que no correspondía. `tools/make-icon.ps1` lo regenera.

### Corregido

- Elegir solo un tema no activaba el botón Guardar.
- Al cambiar de fuente, la barra de volumen podía seguir controlando la app anterior durante 1,5 s.
- El video acoplado se desacoplaba solo cuando el navegador corría su ventana, tironeaba si se lo arrastraba entre dos chequeos y desaparecía al soltarlo sobre la isla colapsada.
- Si otra app había tomado el atajo global, ningún ajuste se podía guardar; los botones rápidos ahora vuelven a su estado si el guardado falla.
- Cambiar cualquier ajuste (o un botón rápido) devolvía la ventana arriba al centro. Ahora solo vuelve ahí al activar la isla o "Mostrar al pasar el ratón"; sin ellos, la app también reaparece donde la dejaste al mostrarla con el atajo o la bandeja.

## [1.0.1] — 2026-11-10

### Corregido

- **Recuadro sombreado alrededor de la tarjeta** en los temas Cristal, OLED, Mica, Neón y Atardecer. La ventana mide 480 px y la tarjeta queda a 6 px de cada borde: la sombra exterior (hasta 30 px) llegaba a esos bordes y se cortaba en seco. Ahora se desvanece en 5 px como máximo hacia los costados y cae hacia abajo; el brillo de Neón y el de la zona para soltar el video también se achicaron.

## [1.0.0] — 2026-11-10

Primera versión estable. Incluye todo lo de 0.2.0 más lo siguiente.

### Isla dinámica y encabezado

- Ventana fija de 480 × 460: solo la tarjeta se transforma, sin el salto lateral al pasar a la isla.
- Botones rápidos en el encabezado: activar/desactivar la isla dinámica, arrastre y estadísticas.
- La isla reproduciendo baja el uso de GPU (7,3 % → 0,3 %).
- Ajustes con pestañas y animaciones pulidas.
- Corregidos 4 bugs de la revisión de las fases 1–6.

### Avisos de notificaciones

- Copia silenciosa de las notificaciones de Windows (para usar con *No molestar*), sin sonido ni robo de foco.

### Video acoplado (PiP del navegador)

- Nuevo `src/pip.rs`: detecta la ventana *Picture in Picture* de Chrome/Edge/Brave/Firefox con `EnumWindows` (clase, `WS_EX_TOPMOST` y título, excluyendo las ventanas propias) y reconoce el arrastre con `GetGUIThreadInfo` (`GUI_INMOVESIZE`) o el cambio de rect, sin hooks ni `GetAsyncKeyState`.
- Al soltarla sobre la tarjeta se acopla: `SetWindowPos` async al hueco, la sigue en `WindowEvent::Moved`, se mantiene encima de la ventana principal y se restaura su rect original al desacoplar, al apagar la opción o al salir desde la bandeja.
- Con la isla colapsada se oculta (`"hide"`) o flota 320 × 180 debajo de la isla (`"float"`). También se oculta mientras Ajustes o el selector de fuentes tapan el reproductor.
- Comandos `pip_set_slot`, `pip_undock` y `pip_status`; eventos `pip-docked` y `pip-drop-hover`. El hilo duerme con la opción apagada o con la ventana oculta y sin video.
- Frontend: diseños `cinema` y `art`, zona para soltar, botón para desacoplar y pestaña *Video* en Ajustes. Las opciones `pip_*` ya se guardan. Las letras se ocultan en modo cine.
- En modo carátula el hueco toma el tamaño real que deja el navegador (su mínimo de PiP es mayor que 260 × 146): evento `pip-size`; si pasa de 300 px de ancho, el título va debajo.
- Correcciones: el video ya no tapa el selector de fuentes, Ajustes entra entero en la ventana en modo carátula y el video flotante ya no aparece un instante en el centro de la pantalla al colapsar.

## [0.2.0] — 2026-10-27

### Fase 1 — Corrección de bugs (`273e494`)

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

### Fase 2 — Isla dinámica real (`f6c1a75`, `f23ac77`)

- Estados `Hidden / Compact / Activity / Expanded` con dueño en el backend (`set_island_mode`): tamaño nativo, posición top-center y evento `island-mode`.
- Morph sin parpadeo: al crecer se agranda la ventana y luego anima el CSS; al encoger anima primero y reduce la ventana 220 ms después. La ventana mide exactamente lo visible.
- Altura animada por JS (36/56 ↔ altura natural) y crossfade entre el reproductor y las vistas compacta/activity.
- Hover con retardo configurable y colapso a los 450 ms; ajustes/popover fijan la isla.
- Actividades en cola (máx. 3, sin duplicados consecutivos, 2,5 s) para pista, fuente y volumen.
- Clic a través opcional en compacta; atajo que alterna expandida ↔ compacta/oculta; fullscreen exclusivo oculta y restaura el modo previo.
- Config nueva: `island_enabled`, `island_always_visible`, `island_activities`, `island_click_through_compact`, `hover_expand_delay_ms`.

### Fase 3 — Estadísticas del sistema (`623eff8`)

- Nuevo `src/sysstats.rs` con un único hilo muestreador, creado de forma perezosa y **bloqueado** cuando las stats están desactivadas o la ventana oculta.
- CPU (`GetSystemTimes`), RAM (`GlobalMemoryStatusEx`), GPU/VRAM/disco (PDH), red (`GetIfTable2`) y temperatura NVIDIA opcional (`nvml.dll` solo si ya existe).
- Evento `system-stats`; primera lectura descartada; query PDH y NVML liberados al desactivar.
- Chips con barra de color, actualización solo de nodos cambiados, resumen opcional en la isla compacta y alertas de CPU/GPU/RAM con histéresis y límite temporal.
- Config nueva: `stats_enabled`, `stats_interval_ms`, `stats_items`, `stats_in_island`, `stats_alerts`.

### Fase 4 — Temas (`a1b4799`)

- `style.css` tokenizado por completo: colores, radios, blurs y sombras solo en bloques de variables.
- 9 temas: `liquid`, `frost`, `oled`, `mica`, `neon`, `sunset`, `minimal`, `contrast` y `auto` (sigue el modo claro/oscuro del sistema en vivo).
- Selector visual de swatches con previsualización inmediata, reversión al cerrar sin guardar y persistencia al confirmar.
- Acento desde la carátula (desactivable); algunos temas lo fijan por diseño. Slider de opacidad con mínimos por tema.
- Config nueva: `theme`, `accent_from_art`, `card_opacity`.
- Botón Guardar con confirmación por clase CSS y `--success` (sin hex inline).

### Fase 5 — Heurística de selección de fuente (`1507e97`)

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
