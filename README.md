# plaggiarism

**Robocopy with super powers** — GUI portable para robocopy. Solo Windows.

## Qué es

Interfaz gráfica sobre robocopy. **No reimplementa robocopy**: lo invoca como subprocess y parsea su stdout para mostrar progreso y resultados en tiempo real.

## Stack

| Capa | Tecnología |
|---|---|
| GUI | Tauri v2 |
| Frontend | React 19 + TypeScript + TailwindCSS (`src/`) |
| Backend | Rust — invoca robocopy y parsea su stdout (`src-tauri/`) |
| Build | pnpm (frontend) + cargo (Rust) |
| Config | JSON con extensión `.plagg` |

## Requisitos

- **Windows** (robocopy es de Windows)
- **Node.js + pnpm**
- **Toolchain Rust** (`rustup` + MSVC build tools)
- WebView2 (incluido en Windows 10/11)

## Levantar el proyecto

```bash
pnpm install        # instala dependencias del frontend

pnpm tauri dev      # desarrollo integrado (frontend + Rust)
pnpm tauri build    # build de distribución (.exe / .msi)
```

Comandos sueltos:

```bash
# Frontend (raíz)
pnpm dev
pnpm build          # tsc && vite build

# Backend (en src-tauri/)
cargo build
cargo test
cargo fmt
cargo clippy
```

## Distribución portable

Plaggiarism se empaqueta como un **ejecutable portable auto-contenido** (sin instalador): el frontend va embebido dentro del `.exe`.

```bash
pnpm tauri build --no-bundle   # produce solo el .exe en src-tauri/target/release/plaggiarism.exe
```

El `.exe` (~9 MB) se copia a un share o a cada equipo y se ejecuta con un acceso directo. La app es **stateless**: no escribe en `%APPDATA%` ni guarda historial/perfiles; la configuración se exporta/importa vía `.plagg`.

### Requisitos en el equipo destino

- **WebView2** (runtime de Edge): preinstalado en Windows 11 y Windows 10 actualizado. Para Windows 7/8 o máquinas sin Edge, instalar el [Evergreen Bootstrapper](https://developer.microsoft.com/microsoft-edge/webview2/).

### Aviso de SmartScreen (sin firma de código)

El `.exe` no está firmado. Windows puede mostrar "Windows protegió su PC" si el archivo fue **descargado por navegador o correo** (por el *Mark of the Web*). Soluciones:

1. Distribuir por **copia de red / robocopy** — no agrega Mark of the Web, así que no aparece el aviso.
2. Si se descargó por navegador: **`Unblock-File .\plaggiarism.exe`** en PowerShell (o botón "Desbloquear" en Propiedades).
3. O pulsar "Ejecutar de todas formas".

En entornos con **AppLocker/GPO** que bloquean ejecución, agregar el `.exe` a la lista permitida con una **regla por hash o por ruta** (no requiere certificado de firma).

## Notas importantes

- **Robocopy no se reimplementa** — se invoca como subprocess y se parsea su stdout.
- El progreso de robocopy es **por archivo individual**, no global; no usarlo como indicador global.

### Modos de copia

| Modo | Flags | Efecto |
|---|---|---|
| Incremental | `/E /W:1 /R:1` | Copia nuevos/modificados. Nunca borra en destino. |
| Mirror | `/MIR /W:1 /R:1` | Destino = espejo exacto de origen. **Puede eliminar archivos.** |
| Scan | `/L` | Solo lista metadata, no copia (pre-validación y conteo). |

### Exit codes → estado visual

| Exit | Significado | Estado |
|---|---|---|
| 0 | Nada que copiar | Neutro |
| 1 | Copiado correctamente | Verde |
| 8 | Algunos archivos fallaron | Amarillo |
| 16 | Error fatal | Rojo |
