# PLAGGIARISM — Documento de Proyecto

> GUI portable para robocopy. Seguridad y claridad ante todo.
> Futura identidad: **plagg.io**

---

## Stack técnico

| Capa | Tecnología |
|---|---|
| Framework GUI | **Tauri v2** |
| Frontend | React + TypeScript + TailwindCSS |
| Backend | Rust (invocación de robocopy, filesystem, streams) |
| Build | pnpm (o cargo + vite en sus respectivos directorios) |
| Config export | JSON con extensión `.plagg` |

**Por qué Tauri:** ejecutable liviano (no Electron), Rust nativo para el proceso hijo, y el frontend es React/TypeScript — stack que ya domina el developer. Robocopy **no se reimplementa** — se invoca como subprocess y se parsea su stdout.

---

## Estructura del repo

```
plaggiarism/
├── src/                  # React + TypeScript (UI completa)
├── src-tauri/            # Rust (Tauri commands, robocopy subprocess)
├── .specs/               # Spec-driven development (SDD)
├── PLAGGIARISM.md        # Este documento
└── AGENTS.md             # Instrucciones para agentes de IA
```

---

## Pantallas y estados

### 1. Pantalla principal

Campos requeridos (todos obligatorios antes de habilitar GO!):

- **Origen** — file picker con indicador visual (verde = seleccionado)
- **Destino** — file picker con indicador visual (rojo = vacío / verde = seleccionado)
- **Modo de copia** — radio buttons, ninguno seleccionado por defecto (requerido uno para avanzar):
  - 🔄 **Copia Incremental** — copia archivos nuevos o modificados. No elimina nada en destino. Ideal para actualizar un respaldo existente. `flags: /E /W:1 /R:1`
  - 🪞 **Copia Espejo (Mirror)** — destino quedará idéntico al origen. Si un archivo fue borrado en origen, también se eliminará en destino. `flags: /MIR /W:1 /R:1`
- **Excluir rutas** — campo `<path>` + botón Examinar + botón Agregar ruta (lista expandible)
- **Command Preview** — caja de solo lectura, estética VS Code Dark+:
  - `robocopy` → color palabra clave (azul/púrpura)
  - Ruta Origen → verde (coherente con el indicador del picker)
  - Ruta Destino → naranja/rojo (coherente con el indicador del picker)
  - Cada flag (`/E`, `/MIR`, `/W:1`, `/R:1`) → color parámetro, con tooltip on-hover explicando qué hace
  - Resumen en lenguaje humano debajo del comando

**Validaciones previas a GO!:**
- ¿Existe la ruta origen y es accesible?
- ¿Existe la ruta destino y es accesible?
- ¿Hay espacio suficiente en destino? (dato obtenido del scan `/L`)
- Si modo Mirror: correr `/L` primero → mostrar modal de confirmación con conteo de archivos que se eliminarían en destino

---

### 2. Pantalla de ejecución (animación)

**Fase 1 — Scan previo (robocopy /L):**
- Si termina en < 8 segundos: mostrar contador de archivos a copiar, comenzar animación
- Si supera 8 segundos sin terminar: mostrar mensaje "Tu respaldo es muy grande, por favor ten paciencia..." mientras sigue el scan

**Fase 2 — Copia activa:**
- Animación **túnel hyperspace/velocidad de la luz** construida con strings, palabras, nombres de archivos, fragmentos de código viajando hacia el centro
- Contador inferior: `Archivos restantes: X de Y` — se decrementa por cada línea de robocopy que indica archivo completado (no % por bytes)
- **Estado normal:** animación en colores fríos (azul, cyan, blanco)
- **Error parcial (exit code 8):** turbulencia roja en el túnel → robocopy salta el archivo → animación retoma velocidad → contador de errores persistente en pantalla (`X archivos con error`)
- **Error fatal (exit code 16):** túnel frena, colapsa a rojo completo, partículas se dispersan hacia afuera, aparece mensaje de error centrado con transición elegante

Todas las transiciones de estado son animadas — **nunca un corte abrupto.**

---

### 3. Pantalla de resultado

Misma estética que el Command Preview (VS Code Dark+). Muestra:

- Modo ejecutado
- Origen → Destino
- Archivos copiados / saltados / fallidos (datos directos del output de robocopy)
- Duración total
- Si hubo errores: listado exacto de archivos fallidos, formateado y legible (no dump crudo)
- Botones: `Exportar resultado` / `Volver al inicio` / `Guardar como perfil`

**Código de salida de robocopy → estado visual:**

| Exit code | Significado | Estado |
|---|---|---|
| 0 | Nada que copiar (destino ya estaba igual) | Neutro |
| 1 | Todo copiado correctamente | Verde — éxito |
| 8 | Algunos archivos fallaron (límite de reintentos) | Amarillo — advertencia |
| 16 | Error fatal — no copió nada | Rojo — error crítico |

---

### 4. Historial y perfiles

- Lista de trabajos ejecutados: fecha, modo, origen → destino, resultado (X copiados, Y fallidos), duración
- Perfiles guardados: cargar con un click, pre-llena toda la pantalla principal
- Acceso desde la pantalla principal (botón o panel lateral)

---

### 5. Gestión de configuraciones (.plagg)

El archivo `.plagg` es un JSON estándar:

```json
{
  "nombre": "Respaldo servidor geología",
  "origen": "D:\\Datos\\Geologia",
  "destino": "E:\\Respaldos\\Geologia",
  "modo": "incremental",
  "flags": { "W": 1, "R": 1 },
  "excluirRutas": ["C:\\Temp", "D:\\Cache"],
  "creadoEn": "2026-07-04T10:30:00"
}
```

Acciones disponibles (antes o después de una copia):
- **Guardar como perfil** → almacenado internamente en la app
- **Exportar .plagg** → el usuario elige dónde guardarlo en disco (para compartir con colegas, guardar en servidor, etc.)
- **Importar .plagg** → file picker, carga todos los parámetros automáticamente en la pantalla principal

---

## Workflow de desarrollo — SDD tradicional

### Modelo de trabajo

Un harness activo a la vez (Claude Code o OpenCode). El cambio entre harnesses se hace por consumo de tokens, no por paralelismo. Ambos leen `.specs/` como fuente de verdad antes de tocar código — esto garantiza continuidad transparente entre sesiones y entre agentes.

**Roles:**

| Actor | Rol |
|---|---|
| Flavio | Gate de specs, review de diffs, merge a main, marca checkboxes en tasks.md |
| Claude Code / OpenCode | Implementación dentro del scope del spec activo |
| `.specs/` | Fuente de verdad — ningún agente modifica specs sin aprobación de Flavio |

### Estructura de specs

```
.specs/
├── PLAGGIARISM.md             # Constitución del proyecto (este documento)
├── spec001-tauri-scaffold/    # Bootstrap Tauri, estructura inicial
│   ├── spec.md
│   ├── plan.md
│   └── tasks.md
├── spec002-ui-main-screen/    # Pantalla principal + Command Preview
├── spec003-rust-robocopy/     # Subprocess robocopy, parser stdout, exit codes
├── spec004-animation/         # Túnel hyperspace + estados de error
└── spec005-config-profiles/   # .plagg export/import + historial
```

Cada spec sigue el flujo:
`spec.md` → `plan.md` → `tasks.md` → implementación → review humano → merge a main

**Regla de escritura en `.specs/`:** los agentes solo leen esta carpeta. Flavio es el único que actualiza estados y marca checkboxes en `tasks.md`.

---

## Scope MVP (v0.1)

**Entra:**
- Pantalla principal funcional (origen, destino, modo, excluir rutas)
- Command Preview estilo VS Code
- Invocación real de robocopy con los flags correctos
- Animación hyperspace básica + estados normal/error
- Contador de archivos restantes (basado en scan `/L`)
- Pantalla de resultado con datos reales de robocopy
- Export/Import `.plagg`

**Queda para v0.2:**
- Historial de trabajos persistente
- Perfiles guardados internamente
- Modal de confirmación para Mirror con listado de archivos a eliminar
- Tooltips en flags del Command Preview
- Detección de espacio insuficiente en destino

---

## Notas técnicas importantes

- Robocopy **sin `/MIR`** nunca borra nada en destino — este es el comportamiento seguro por defecto (modo Incremental)
- **`/MIR`** hace que destino sea espejo exacto de origen — puede eliminar archivos. Requiere confirmación explícita
- El flag `/L` (list-only) no lee contenido de archivos, solo metadata. La velocidad del scan depende de cantidad de archivos/carpetas, no de los GB totales
- Timeout de scan: si `/L` supera 8 segundos, mostrar mensaje de paciencia — no bloquear la UI
- En Rust los imports son `use` — no `import` ni `require`
- El porcentaje de avance de robocopy es **por archivo individual**, no global — no usar como indicador de progreso global

---

*Documento generado en sesión de diseño con Niag-01 — 2026-07-04*