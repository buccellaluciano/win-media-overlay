# 🎵 WMO (Win11 Media Overlay)

Una **isla dinámica** flotante, elegante y ultraligera para controlar la reproducción de música en Windows 11, con estética *Liquid Glass* de iOS / VisionOS y estadísticas del sistema integradas.

Construida con **Tauri 2 + Rust + HTML/CSS/JS vanilla**, compilada **100 % offline** con las librerías locales del equipo. Sin frameworks JS, sin npm, sin CDNs.

**Versión 1.0.1.**

---

## ✨ Características

### Isla dinámica real

La ventana no es una tarjeta fija: es una cápsula que **se transforma** entre estados, con morph de tamaño, radio y contenido.

| Estado | Tamaño | Qué muestra |
|---|---|---|
| `compact` | 240 × 36 | Mini carátula, título con marquee y ecualizador animado (solo reproduciendo) |
| `activity` | 340 × 56 | Aviso transitorio de 2,5 s: cambio de pista, cambio de fuente o volumen |
| `expanded` | 468 × 176+ | Reproductor completo (controles, scrubber, volumen, letras, ajustes y stats) |

- **Hover**: la compacta se expande tras `250 ms` (configurable 0–1000 ms); al salir del área vuelve a compacta a los `450 ms`.
- **Clic** en la compacta la expande.
- **Atajo global** alterna expandida ↔ compacta/oculta según la configuración.
- **Ventana fija de 480 × 460**: solo la tarjeta se transforma dentro; la ventana nativa nunca cambia de tamaño (redimensionar un WebView mostraba un cuadro desplazado en cada transición).
- **Sin área transparente que bloquee clics**: todo lo que no es la tarjeta es clic-a-través; el frontend informa su rectángulo y el backend activa/desactiva el paso de clics según el cursor.
- **Actividades**: cola de hasta 3 avisos, sin duplicados consecutivos, 2,5 s cada uno.

### Integración con la API universal de Windows (WinRT GSMTC)

- Compatible con **Spotify, Tidal, Apple Music, YouTube (Chrome, Edge, Firefox, Brave), VLC, foobar2000**, etc.
- Lectura en tiempo real de pista, artista, álbum, portada, posición y duración.
- Controles: Play/Pausa, Siguiente, Anterior y scrubber interactivo.
- **Heurística de selección de fuente**: puntúa cada sesión (reproduciendo, sesión mostrada, título/duración, sesión actual de Windows) y descarta anuncios/previews recién aparecidos o sonidos muy cortos. Si la fuente mostrada se pausa y otra empieza a sonar, cambia en ~1,5 s; si nadie suena, se queda la mostrada.
- **Selección manual** desde el popover de fuentes (incluye “Automático (Windows 11)”), estable aunque cambie el orden de las sesiones o se abran/cierren pestañas.
- **Volumen por aplicación** (WASAPI): cada app conserva su nivel; se re-aplica al cambiar de pista o hacer seek. Si no hay sesión de audio, usa el volumen del sistema.
- **Letras sincronizadas (opt-in)**: fuente pública **LRCLIB**; se envían solo título, artista, álbum y duración. Timeout de 6 s y caché acotada.

### Estadísticas del sistema (opt-in)

- **CPU** (`GetSystemTimes`), **RAM** (`GlobalMemoryStatusEx`), **GPU 3D** y **VRAM** (PDH), **red** ↓/↑ (`GetIfTable2`) y **uso de disco** (PDH).
- **Temperatura GPU solo en NVIDIA**: se usa `nvml.dll` *si ya existe* en el sistema (la instala el driver). Nunca se descarga ni se incluye en el repo. En AMD/Intel la métrica simplemente no aparece.
- Chips compactos con mini barra verde (< 60 %), ámbar (< 85 %) y roja (≥ 85 %).
- Intervalo configurable (0,5–5 s). Resumen opcional en la isla compacta (alterna cada 4 s).
- Alertas opcionales: CPU/GPU > 95 % sostenido 10 s o RAM > 90 % → aviso 2,5 s (máximo uno por métrica cada 5 min).
- **Un único hilo muestreador** que duerme por completo cuando las stats están desactivadas o la ventana está oculta.

### Avisos de notificaciones (opt-in)

Deja Windows en **No molestar** y entérate igual: WMO muestra una copia **silenciosa** de cada notificación nueva arriba a la derecha, **sin sonido y sin quitarle el foco** a la app o al juego.

- Lee el centro de notificaciones con la API pública `UserNotificationListener` (con el permiso de Windows en *Configuración › Privacidad y seguridad › Notificaciones*).
- Sin empaquetar (MSIX) Windows no deja suscribirse al evento de cambios, así que se consulta cada 1,5 s: el aviso llega con ese retraso como máximo.
- Ícono de la app que envía la notificación: logo de la Store en apps empaquetadas y, en apps de escritorio (Discord, Teams clásico…), el mismo ícono del menú Inicio vía `shell:AppsFolder`.
- Pila de hasta 3 avisos (el más nuevo arriba), duración configurable (4/6/10 s), se pausan al pasar el ratón; clic o ✕ para descartar.
- Privacidad: opción para mostrar solo la app y el título, y lista de apps a ignorar.
- No se muestran mientras hay un juego en pantalla completa exclusiva.
- Limitaciones: solo notificaciones que pasan por el sistema de Windows; los botones de la notificación original (p. ej. *Responder*) no se pueden usar desde el aviso.

### Video acoplado (PiP del navegador)

Arrastrá la ventana *imagen en imagen* de Chrome, Edge, Brave o Firefox y soltala sobre la tarjeta: el video queda **acoplado** dentro del reproductor y lo sigue cuando movés la app.

- Es la **ventana real del navegador**, no una captura: no consume más, funciona con DRM (Netflix…) y sus controles siguen andando.
- Dos diseños: **Modo cine** (16:9 a todo el ancho, con una barra mínima de título y controles debajo) o **En lugar de la carátula** (260 × 146, ideal para videos musicales).
- Con la isla colapsada el video se **oculta** o queda **flotando** debajo de la isla.
- Para desacoplar: arrastralo fuera de la tarjeta (recupera su tamaño) o usá el botón del encabezado. Al salir desde la bandeja vuelve a su lugar.
- Se configura en *Ajustes › Video*.

### Temas y apariencia

- **9 temas** con previsualización inmediata y guardado al confirmar:
  `Liquid Glass`, `Cristal`, `OLED`, `Mica`, `Neón`, `Atardecer`, `Minimal`, `Alto contraste` y `Automático` (sigue el modo claro/oscuro de Windows en vivo).
- Todos los colores, radios, sombras y desenfoques son **tokens CSS**; cada tema solo redefine variables.
- **Acento según la carátula** (desactivable); algunos temas fijan su propio acento para garantizar contraste.
- **Opacidad** de la tarjeta ajustable (40–100 %), con mínimos por tema para no romper la legibilidad.
- Botón de guardado con confirmación usando el color de éxito del tema.

### Comportamiento y atajos

- **Icono en la bandeja**: clic izquierdo muestra/oculta; clic derecho → *Mostrar / Ocultar* y **Salir**.
- **Atajo global** por defecto `Ctrl + Shift + Espacio`, configurable (modificadores + tecla) con `RegisterHotKey`. Si otra app lo tiene registrado, se avisa en rojo bajo el bind y se conserva el atajo anterior.
- **Respetar pantalla completa**: en fullscreen exclusivo (D3D) la isla se oculta para no minimizar el juego y el atajo muestra sin robar el foco.
- **Botones rápidos del encabezado**: activar/desactivar la isla dinámica, permitir arrastrar, estadísticas y mostrar al pasar el ratón, sin abrir Ajustes. Con un video acoplado aparece también el botón para desacoplarlo.
- **Escape**: cierra el popover/ajustes o colapsa la isla (según el estado).
- **Auto-hide on blur** (solo en el modo clásico sin isla).
- Sin bordes, sin icono en la barra de tareas (`skipTaskbar`).

---

## 🚀 Cómo ejecutarlo

**Versión portable (recomendada):** descargá `WMO-1.0.1-win64.zip` desde *Releases*, descomprimilo y hacé doble clic en `Iniciar.bat` (o en `wmo.exe`). No requiere instalación.

**Desde el código fuente** (después de `cargo build --release --offline`):

1. **Doble clic en `run.bat`** en la carpeta del proyecto.
2. O ejecuta el binario compilado:
   ```powershell
   .\target\release\wmo.exe
   ```

Pulsa **`Ctrl + Shift + Espacio`** para mostrar u ocultar la interfaz en cualquier momento.

> Requiere Windows 11 con **WebView2** (viene preinstalado en Windows 11).

---

## ⚙️ Configuración

Engranaje (esquina superior derecha) → panel de ajustes:

1. **Tema**: rejilla de swatches con previsualización inmediata (se revierte si cierras sin guardar), acento por carátula y slider de opacidad.
2. **Atajo de teclado**: modificadores y tecla, con detección de conflictos.
3. **Isla dinámica**: activar, compacta siempre visible, actividades y retardo de expansión.
4. **Estadísticas del sistema**: activar, intervalo, métricas, resumen en la isla y alertas.
5. **Comportamiento**: arrastre, resplandor ambiental, auto-ocultado, volumen por aplicación, respeto de pantalla completa y letras.
6. **Guardar Preferencias** aplica y persiste los cambios.

- Configuración: `%APPDATA%\WMO\config.json` (se valida y se guarda de forma atómica; si el JSON está corrupto se conserva como `config.json.bak-<timestamp>`).
- Logs y pánicos: `%APPDATA%\WMO\logs\` (nunca en el directorio de trabajo).

---

## 🛡️ Seguridad, anticheat y antivirus

WMO está pensado para convivir con juegos que usan **EAC, BattlEye, Vanguard o FACEIT**. En lenguaje llano, **el programa no hace**:

- ❌ **No simula teclado ni ratón** (`keybd_event`, `SendInput`, `mouse_event`): no hay macros ni entrada sintética.
- ❌ **No instala hooks globales** (`SetWindowsHookEx`): no hay keyloggers ni hooks de ratón. El cursor se consulta con `GetCursorPos` en polling.
- ❌ **No abre otros procesos** para leer su memoria (`OpenProcess`, `ReadProcessMemory`, `WriteProcessMemory`) ni inyecta DLLs.
- ❌ **No engancha D3D/DXGI** ni mide FPS del juego.
- ❌ **No carga drivers de kernel** (WinRing0, LibreHardwareMonitor, inpout, PawnIO…) ni pide permisos de administrador.
- ❌ **No enumera procesos** del sistema (`CreateToolhelp32Snapshot`, `EnumProcesses`, `sysinfo`).
- ❌ **No empaqueta con packers** ni ofusca el binario.
- ❌ **No usa WMI** en bucles ni crea servicios/tareas programadas.
- 🌐 **Única conexión de red**: `https://lrclib.net`, solo si activas las letras (opt-in). Se envían título, artista, álbum y duración.

Lo que **sí** usa son APIs públicas de Windows en modo usuario: GSMTC, WASAPI, PDH, `GetSystemTimes`, `GlobalMemoryStatusEx`, `GetIfTable2`, `RegisterHotKey`, `GetCursorPos`, `UserNotificationListener` (solo con los avisos activados), `EnumWindows` + `SetWindowPos`/`ShowWindowAsync` **solo sobre la ventana PiP del navegador** para el video acoplado (enumera ventanas, no procesos; sin hooks ni `SetParent`) y, opcionalmente, `nvml.dll` si el driver de NVIDIA ya la instaló.

**Limitación de fullscreen exclusivo**: Windows no permite dibujar un overlay encima de una app D3D exclusiva sin minimizarla. En ese caso la isla se oculta y el atajo la muestra sin foco, pero puede quedar detrás. Para verla siempre, usa **pantalla completa sin bordes** en el juego.

---

## 🔏 Firma de código

El proyecto no incluye firma digital. Windows SmartScreen y las heurísticas de reputación puntúan mejor un ejecutable firmado, así que si distribuyes `wmo.exe` se recomienda firmarlo con un **certificado de firma de código** (estándar o EV) o con **Azure Trusted Signing**. Firmar no cambia el comportamiento de la app: solo mejora la confianza del sistema y de los antivirus.

---

## 🛠️ Desarrollo

Compilación y verificación **siempre offline**:

```powershell
cargo build --release --offline
cargo clippy --offline
cargo test --offline
```

Tests incluidos:
- `media::tests::*`: heurística de selección de fuente.
- `sysstats::tests::samples`: lectura real de CPU/RAM/GPU/red/disco.

No se descargan crates, paquetes ni recursos externos. Las features nuevas del crate `windows` ya están en `Cargo.lock`.

---

## 📁 Estructura del Proyecto

```
wmo/
├── Cargo.toml          # Dependencias, features del crate windows y perfil release
├── Cargo.lock          # Versiones exactas (compilación offline)
├── build.rs            # Script de compilación de Tauri
├── tauri.conf.json     # Ventana, CSP estricta y metadatos del bundle
├── LICENSE             # Licencia MIT
├── CHANGELOG.md        # Historial de cambios
├── PIP_DOCK.md         # Diseño del video acoplado (PiP)
├── packaging/          # Iniciar.bat y LEEME.txt del zip de la release
├── icons/              # Iconos de la aplicación
├── capabilities/       # Permisos ACL de Tauri 2
├── run.bat             # Lanzador rápido
├── src/
│   ├── main.rs         # Inicialización, IPC, estado de la isla, tray y watchers
│   ├── media.rs        # GSMTC: selección de sesión, estado, carátula y controles
│   ├── audio_session.rs# Volumen por aplicación vía WASAPI (mezclador de Windows)
│   ├── volume.rs       # Volumen maestro del sistema (IAudioEndpointVolume)
│   ├── sysstats.rs     # Muestreador de CPU/RAM/GPU/VRAM/red/disco y NVML opcional
│   ├── notifications.rs# Avisos: lee el centro de notificaciones y muestra copias silenciosas
│   ├── pip.rs          # Video acoplado: detecta la ventana PiP del navegador y la ubica en la tarjeta
│   ├── fullscreen.rs   # Detección de fullscreen exclusivo (D3D) en primer plano
│   ├── hotkey.rs       # RegisterHotKey con hilo propio y mensajes bloqueantes
│   ├── position.rs     # Posicionado top-center por monitor y DPI
│   └── config.rs       # AppConfig, validación y persistencia atómica
└── ui/
    ├── index.html      # Isla compacta/activity, reproductor, letras y ajustes
    ├── style.css       # Tokens + 9 temas (Liquid, Frost, OLED, Mica, Neón…)
    ├── app.js          # Lógica frontend, morph de la isla, stats y temas
    ├── notify.html     # Ventana de avisos (arriba a la derecha, nunca toma el foco)
    ├── notify.css      # Estilo de los avisos (reusa los tokens de tema)
    └── notify.js       # Pila de avisos, temporizadores y tamaño de la ventana
```

---

## 📄 Licencia

[MIT](LICENSE) © 2026 buccellaluciano.
