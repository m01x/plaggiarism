# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Fuente de verdad — SDD

**Antes de cualquier acción:** leer `.specs/PLAGGIARISM.md` y el spec activo que indique el developer.

Reglas operacionales:
- `.specs/` es la única fuente de verdad. Si hay conflicto entre conocimiento propio y el spec, el spec gana.
- No modificar ningún archivo dentro de `.specs/`.
- No marcar checkboxes en `tasks.md` — eso lo hace el developer (Flavio).
- Un harness activo a la vez. No hay trabajo en paralelo entre specs.
- Si el spec activo no está claro, preguntar antes de implementar.

## Proyecto

`plaggiarism` — GUI portable para robocopy ("Robocopy with super powers"). Futura identidad: **plagg.io**. Solo Windows.

## Stack

| Capa | Tecnología |
|---|---|
| Framework GUI | Tauri v2 |
| Frontend | React + TypeScript + TailwindCSS (`src/`) |
| Backend | Rust — invocación de robocopy como subprocess (`src-tauri/`) |
| Build | pnpm (frontend) + cargo (Rust) |
| Config export | JSON con extensión `.plagg` |

**Robocopy no se reimplementa** — se invoca como subprocess y se parsea su stdout.

## Comandos

Frontend:
```
pnpm install
pnpm dev          # dev server (Tauri abre la ventana)
pnpm build
```

Rust / Tauri:
```
cargo build
cargo test
cargo test <test_name> -- --exact
cargo fmt
cargo clippy
cargo tauri dev   # inicia app completa en modo desarrollo
cargo tauri build # build de distribución
```

## Arquitectura

```
plaggiarism/
├── src/           # React + TypeScript (UI completa)
├── src-tauri/     # Rust (Tauri commands, robocopy subprocess, parser)
└── .specs/        # Fuente de verdad — solo lectura para agentes
```

El backend Rust expone Tauri commands al frontend. El flujo central:
1. Frontend construye parámetros → invoca Tauri command
2. Rust lanza `robocopy` como subprocess con los flags correspondientes
3. Rust parsea stdout línea a línea → emite eventos al frontend (archivos completados, errores)
4. Frontend actualiza UI en tiempo real (animación, contador)

Flags de robocopy:
- **Incremental:** `/E /W:1 /R:1` — copia nuevos/modificados, nunca borra
- **Mirror:** `/MIR /W:1 /R:1` — destino = espejo exacto de origen, puede eliminar archivos
- **Scan:** `/L` — solo lista metadata, no copia (usado para pre-validación y conteo)

Exit codes relevantes de robocopy: `0` = nada que copiar, `1` = éxito, `8` = errores parciales, `16` = error fatal.

## Notas técnicas

- El porcentaje que emite robocopy es **por archivo individual**, no global — no usarlo como indicador de progreso global.
- El flag `/L` no lee contenido de archivos, solo metadata — la velocidad depende de cantidad de archivos, no de GB.
- En Rust los imports son `use` — no `import` ni `require`.
- Rutas Windows en el `.plagg` JSON usan `\\`.
