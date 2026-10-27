# 🎵 WMO (Win11 Media Overlay)

Una **isla dinámica** flotante, elegante y ultraligera para controlar la reproducción de música en Windows 11, con estética *Liquid Glass* de iOS / VisionOS y estadísticas del sistema integradas.

Construida con **Tauri 2 + Rust + HTML/CSS/JS vanilla**, compilada **100 % offline** con las librerías locales del equipo. Sin frameworks JS, sin npm, sin CDNs.

**Versión 0.2.0.**

---

## ✨ Características

### Isla dinámica real

La ventana no es una tarjeta fija: es una cápsula que **se transforma** entre estados, con morph de tamaño, radio y contenido.

| Estado | Tamaño | Qué muestra |
|---|---|---|
| `compact` | 240 × 36 | Mini carátula, título con marquee y ecualizador animado (solo reproduciendo) |
| `activity` | 340 × 56 | Aviso transitorio de 2,5 s: cambio de pista, cambio de fuente o volumen |
| `expanded` | 480 × 188+ | Reproductor completo (controles, scrubber, volumen, letras, ajustes y stats) |

- **Hover**: la compacta se expande tras `250 ms` (configurable 0–1000 ms); al salir del área vuelve a compacta a los `450 ms`.
- **Clic** en la compacta la expande.
- **Atajo global** alterna expandida ↔ compacta/oculta según la configuración.
- **Clic a través** opcional en compacta: los clics atraviesan la cápsula hasta que el cursor entra en ella.
- **Sin área transparente que bloquee clics**: la ventana nativa mide exactamente lo que se ve.
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
- **Escape**: cierra el popover/ajustes o colapsa la isla (según el estado).
- **Auto-hide on blur** (solo en el modo clásico sin isla).
- Sin bordes, sin icono en la barra de tareas (`skipTaskbar`).

---

## 🚀 Cómo ejecutarlo

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
3. **Isla dinámica**: activar, compacta siempre visible, actividades, clic a través y retardo de expansión.
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

Lo que **sí** usa son APIs públicas de Windows en modo usuario: GSMTC, WASAPI, PDH, `GetSystemTimes`, `GlobalMemoryStatusEx`, `GetIfTable2`, `RegisterHotKey`, `GetCursorPos` y, opcionalmente, `nvml.dll` si el driver de NVIDIA ya la instaló.

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
├── AGENTS.md           # Reglas para agentes que trabajen en el repo
├── TAREAS_DEEPSEEK.md  # Especificación por fases
├── CHANGELOG.md        # Historial de cambios
├── icons/              # Iconos de la aplicación
├── capabilities/       # Permisos ACL de Tauri 2
├── run.bat             # Lanzador rápido
├── src/
│   ├── main.rs         # Inicialización, IPC, estado de la isla, tray y watchers
│   ├── media.rs        # GSMTC: selección de sesión, estado, carátula y controles
│   ├── audio_session.rs# Volumen por aplicación vía WASAPI (mezclador de Windows)
│   ├── volume.rs       # Volumen maestro del sistema (IAudioEndpointVolume)
│   ├── sysstats.rs     # Muestreador de CPU/RAM/GPU/VRAM/red/disco y NVML opcional
│   ├── fullscreen.rs   # Detección de fullscreen exclusivo (D3D) en primer plano
│   ├── hotkey.rs       # RegisterHotKey con hilo propio y mensajes bloqueantes
│   ├── position.rs     # Posicionado top-center por monitor y DPI
│   └── config.rs       # AppConfig, validación y persistencia atómica
└── ui/
    ├── index.html      # Isla compacta/activity, reproductor, letras y ajustes
    ├── style.css       # Tokens + 9 temas (Liquid, Frost, OLED, Mica, Neón…)
    └── app.js          # Lógica frontend, morph de la isla, stats y temas
```
