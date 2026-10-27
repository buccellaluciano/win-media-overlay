# 🎵 WMO (Win11 Media Overlay)

Una interfaz flotante elegante, ultraligera y moderna estilo **Liquid Glass de iOS / VisionOS** para controlar la reproducción de música en Windows 11, reemplazando la tosca barra de tareas.

Construida con **Tauri 2 + Rust + Vanilla HTML5/CSS3/JS**, compilada 100% offline utilizando únicamente las librerías locales de tu equipo sin realizar descargas externas.

---

## ✨ Características Principales

- **Diseño iOS Liquid Glass (Acrílico / Cristal)**:
  - Fondo translúcido con desenfoque extremo (`backdrop-filter: blur(45px)`).
  - Resplandor ambiental dinámico (*Liquid Glow*) que proyecta los colores de la carátula activa hacia el fondo de la pantalla.
  - Curvatura suave y bordes especulares con reflejo de luz superior.
  - Carátula del álbum en squircle redondeado con transiciones suaves.
  - Indicador pill estilo *Dynamic Island*.
- **Integración con la API Universal de Windows (WinRT GSMTC)**:
  - Compatible con cualquier reproductor que soporte controles multimedia: **Spotify, Tidal, Apple Music, YouTube (Chrome, Edge, Firefox, Brave), VLC, Foobar2000, etc.**
  - Lectura en tiempo real de pista, artista, álbum, portada y posición de tiempo.
  - Control total: Play/Pausa, Pista Siguiente, Pista Anterior y Scrubber interactivo para saltar de posición en la canción.
- **Atajo Global de Teclado (Bind Personalizable)**:
  - Atajo por defecto: **`Ctrl + Shift + Espacio`** (o el que elijas).
  - Configurable desde el panel de ajustes de la aplicación con modificadores (`Ctrl`, `Shift`, `Alt`, `Win`) y tecla deseada.
  - Se registra nativamente a nivel de sistema operativo con Win32 `RegisterHotKey`.
- **Posición Dynamic Island**: arriba al centro, flotando como una cápsula. Interruptor para permitir o bloquear el arrastre: si la arrastrás, se mueve momentáneamente y vuelve a centrarse arriba (en el monitor donde esté) al mostrarse de nuevo.
- **Opciones de Comportamiento**:
  - **Icono en la bandeja del sistema (tray)**: clic izquierdo muestra/oculta; clic derecho → *Mostrar / Ocultar* y **Salir** (cierra la app por completo).
  - **Revelar al pasar el ratón**: icono en la barra superior (a la izquierda del engranaje). Al activarlo, la app se oculta arriba al centro y reaparece al posar el cursor en la franja superior.
  - **Respetar pantalla completa**: la app queda siempre encima (juegos borderless, ventanas, etc.), pero se aparta automáticamente solo cuando hay una app en **fullscreen exclusivo** (D3D) en primer plano, para no minimizarla; vuelve al salir. En fullscreen exclusivo Windows no permite dibujar el overlay encima sin minimizar el juego: en ese caso se muestra sin robar el foco y puede quedar detrás. Para verlo siempre, usa **pantalla completa sin bordes**.
  - **Letras sincronizadas (opt-in)**: muestra la letra en tiempo real bajo el reproductor, con la línea activa resaltada. Fuente: **LRCLIB** (público, sin cuenta). Requiere internet; solo se envían título, artista, álbum y duración. El panel se colapsa solo si la pista no tiene letra sincronizada, y hay una **flecha** en la barra superior para abrirlo/cerrarlo cuando existen letras.
  - Opción de autocerrado/ocultamiento al hacer clic fuera (*Auto-hide on blur*).
  - **Volumen por aplicación** usando el mezclador de Windows (WASAPI): cada app (Tidal, Spotify, Chrome…) conserva su propio nivel. El valor se re-aplica automáticamente al cambiar de pista o hacer seek (los reproductores recrean su sesión de audio y la resetean).
  - Tecla `Escape` para cerrar al instante.
  - Sin bordes de ventana molestos, sin icono estorbando en la barra de tareas (*skipTaskbar*).

---

## 🚀 Cómo Ejecutarlo

Puedes iniciar la aplicación directamente de dos formas:

1. **Haciendo doble clic en `run.bat`** dentro de la carpeta del proyecto.
2. O ejecutando directamente el binario compilado:
   ```powershell
   .\target\release\wmo.exe
   ```

Una vez abierto, presiona **`Ctrl + Shift + Espacio`** para mostrar u ocultar la interfaz en cualquier momento, incluso si estás jugando o navegando en otra ventana.

---

## ⚙️ Configuración y Personalización

Haz clic en el icono del engranaje en la esquina superior derecha del widget para abrir el panel de configuración:
1. **Atajo de teclado**: Selecciona qué combinación de teclas quieres usar.
2. **Comportamiento**: arrastre, resplandor ambiental, auto-ocultado, volumen por aplicación, *Respetar pantalla completa* y *Letras sincronizadas*.
3. Pulsa **Guardar Preferencias** para aplicar los cambios de inmediato. La configuración se guarda de forma persistente en `%APPDATA%\WMO\config.json`.

En la barra superior, a la izquierda del engranaje, está el icono de **revelar al pasar el ratón** (se activa/desactiva al instante). El icono de **flecha** aparece solo cuando la pista actual tiene letras y sirve para abrir/cerrar el panel de letras.

---

## 🔏 Firma de código

El proyecto no incluye firma digital. Windows SmartScreen y las heurísticas de reputación puntúan mejor un ejecutable firmado, así que si distribuyes `wmo.exe` se recomienda firmarlo con un **certificado de firma de código** (estándar o EV) o con **Azure Trusted Signing**. Firmar no cambia el comportamiento de la app: solo mejora la confianza del sistema y de los antivirus.

---

## 📁 Estructura del Proyecto

```
wmo/
├── Cargo.toml          # Configuración de Rust y dependencias WinRT / Tauri
├── build.rs            # Script de compilación de Tauri
├── tauri.conf.json     # Configuración de ventana (transparente, sin bordes)
├── icons/              # Iconos de la aplicación
├── capabilities/       # Permisos ACL de Tauri 2
├── run.bat             # Lanzador rápido
├── src/
│   ├── main.rs         # Inicialización, IPC y bucle de eventos
│   ├── media.rs        # Controlador WinRT GSMTC (lectura de carátulas y reproducción)
│   ├── audio_session.rs# Volumen por aplicación vía WASAPI (mezclador de Windows)
│   ├── volume.rs       # Volumen maestro del sistema (IAudioEndpointVolume)
│   ├── fullscreen.rs   # Detección de fullscreen exclusivo (D3D) en primer plano
│   ├── hotkey.rs       # Gestor Win32 RegisterHotKey en segundo plano
│   ├── position.rs     # Algoritmo de cálculo de posiciones de pantalla
│   └── config.rs       # Persistencia y gestión de configuración JSON
└── ui/
    ├── index.html      # Estructura del widget y panel de ajustes
    ├── style.css       # Estilos Liquid Glass iOS / VisionOS
    └── app.js          # Lógica frontend y comunicación en tiempo real
```
