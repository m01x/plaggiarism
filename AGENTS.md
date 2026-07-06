# AGENTS.md

## Fuente de verdad

- **Lee `.specs/PLAGGIARISM.md` antes de cualquier acción.** Es la constitución del proyecto.
- Lee el spec activo que te indique el developer (`.specs/specNNN-nombre/`) antes de tocar código.
- `.specs/` es la **única** fuente de verdad. Si hay conflicto entre lo que "sabes" y el spec, el spec gana.
- **No modifiques ningún archivo dentro de `.specs/`.** Solo lectura. Flavio es el único que actualiza specs y `tasks.md`.
- **No marques checkboxes en `tasks.md`** — eso lo hace el developer.
- Un harness activo a la vez (Claude Code o OpenCode). **No hay trabajo en paralelo.** El cambio de harness es por consumo de tokens, no por concurrencia.
- Si el spec activo no está claro, **pregunta antes de implementar.**

## Project

`plaggiarism` — GUI portable para robocopy ("Robocopy with super powers"). Seguridad y claridad ante todo. Futura identidad: plagg.io. **Windows-only** (robocopy es de Windows).

Stack (definido en `.specs/PLAGGIARISM.md`):
- GUI: **Tauri v2**
- Frontend: **React + TypeScript + TailwindCSS** (`src/`)
- Backend: **Rust** — invoca robocopy como subprocess y parsea su stdout (`src-tauri/`)
- Build: `pnpm` (frontend) y `cargo` (backend),ificados en sus directorios
- Config export: JSON con extensión `.plagg`

**Robocopy no se reimplementa** — se invoca como subprocess. El progreso de robocopy es *por archivo individual*, no global — no usar como indicador global.

## State

El repo está en su commit inicial. Solo existen `README.md`, `.gitignore` y `.specs/PLAGGIARISM.md`. No hay `src/`, `src-tauri/`, `package.json`, ni `Cargo.toml` todavía. Los specs previstos (spec001…spec005) aún no están creados como directorios. El primer paso real es el bootstrap Tauri (spec001).

## Comandos

Una vez inicializado el scaffold (post-spec001):
- Frontend (`src/`): `pnpm install`, `pnpm dev`, `pnpm build`, `pnpm lint` (typical Vite/TS setup)
- Backend (`src-tauri/`): `cargo build`, `cargo test`, `cargo fmt`, `cargo clippy`
- Desarrollo integrado: `pnpm tauri dev` / `pnpm tauri build`
- Ningún `Cargo.toml`/`package.json` existe aún; confirma los scripts reales antes de asumir.

`.gitignore` ya ignora `target/`, `debug`, `*.rs.bk` (rustfmt), `*.pdb` (MSVC), y `**/mutants.out*/` (cargo-mutants).

## Códigos de salida de robocopy (mapeo UI)

| Exit | Significado | Estado visual |
|---|---|---|
| 0 | Nada que copiar | Neutro |
| 1 | Copiado correctamente | Verde |
| 8 | Algunos archivos fallaron | Amarillo |
| 16 | Error fatal | Rojo |

## Cuando actualices este archivo

Mantén solo hechos verificables y de alto valor que un agente probablemente erraría sin ayuda. Si algo queda obsoleto, prefíere borrarlo antes de conservar reclamos no verificables.