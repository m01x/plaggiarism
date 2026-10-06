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
