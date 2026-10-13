# Spec 004 — Animation Engine + Result Screen + Export `.plagg`

## Propósito

Cerrar el flujo punta a punta de Plaggiarism: GO! → scan previo → (confirmación Mirror) → copia real con animación → pantalla de resultado, más export/import de configuraciones `.plagg`. Al cerrar este spec, la aplicación copia archivos reales con la identidad visual completa (túnel hyperspace + pantalla de resultado estilo VS Code Dark+).

## Scope

### Backend (ajustes derivados de spec003)

- `build_scan_args` honra `modo` (Mirror → `/MIR /L`) y `excluir` (`/XD`).
- `scan_robocopy` retorna `extrasCount` (columna "Extras" del resumen) además de `fileCount`.
- `run_robocopy` extrae `skipped` (columna "Omitido" del resumen) — hoy siempre es 0.
- Comandos Rust `write_text_file` / `read_text_file` (std::fs) para leer/escribir el `.plagg` en disco (sin `tauri-plugin-fs` ni scoping).

### Frontend

- Hook `useCopyProcess.ts` — máquina de estados que consume el contrato real (incluye `poll_scan`).
- `CopyScreen.tsx` — animación túnel hyperspace (Canvas) + contador + botón Cancelar.
- `ResultScreen.tsx` — datos reales de `RobocopyResult`, ícono por exit code, lista de fallidos.
- Export/Import `.plagg` (diálogo `save`/`open` + `writeTextFile`/`readTextFile` + validación).
- Modal de confirmación Mirror (cuando `extrasCount > 0`).

## Fuera de scope

- Historial persistente de trabajos (spec005).
- Perfiles guardados internamente (spec005).
- Tooltips en flags del Command Preview (spec005).
- Detección de espacio insuficiente en destino (v0.2) → por eso `totalBytes` queda diferido.

## Contrato de API (ya implementado — referencia)

| Invocación | Resultado |
|---|---|
| `validate_paths({ origen, destino })` | `{ origenOk, destinoOk }` |
| `scan_robocopy({ params })` | `{ fileCount, totalBytes, slow, extrasCount }` *(extrasCount: nuevo)* |
| `poll_scan()` | `{ fileCount, totalBytes, slow } \| null` |
| `run_robocopy({ params, totalFiles })` | `RobocopyResult` |
| `cancel_robocopy()` | `void` |

Eventos emitidos por Rust durante `run_robocopy`:

```
'file_completed'  → { remaining: number, fileName: string }
'copy_error'      → { fileName: string, errorCode: number }
'copy_done'       → RobocopyResult
```

`RobocopyResult` = `{ status, copied, skipped, failed, failedFiles, durationSecs }`.

## Plan técnico

### 1. Backend

**`build_scan_args(origen, destino, modo, excluir)`** — firma ampliada. Incremental sigue `/L /E /NFL /NDL /NJH`; Mirror usa `/L /MIR /NFL /NDL /NJH`; agrega `/XD <ruta>...` si hay exclusiones.

**`scan_robocopy`** — extrae `extrasCount` de la fila "Archivos" del resumen (6ª columna, `nums[5]`). Retorna `ScanResult { fileCount, totalBytes, slow, extrasCount }`.

**`run_robocopy_core`** — además de `copied`/`failed`, captura `skipped` de la fila "Archivos" (columna "Omitido", `nums[2]`). Verificar el cómputo de `copied` (no doble-descontar `failed`).

**I/O de archivos** — comandos `write_text_file(path, content)` y `read_text_file(path)` en Rust (std::fs), registrados en `lib.rs`. El diálogo de ruta lo aporta `plugin-dialog` (ya presente). Evita el scoping/capacidades de `tauri-plugin-fs`.

### 2. Máquina de estados (`useCopyProcess.ts`)

```typescript
type CopyState =
  | { status: 'idle' }
  | { status: 'scanning' }
  | { status: 'scanning_slow' }              // scan > 8s → poll_scan hasta final
  | { status: 'copying'; remaining: number; total: number; lastFile: string; errorCount: number }
  | { status: 'fatal'; message: string }
  | { status: 'done'; result: RobocopyResult }
```

Flujo:
1. GO! → `scan_robocopy`.
2. Si `slow` → estado `scanning_slow` + bucle `poll_scan` hasta `slow: false` (resultado final).
3. Si `modo === 'mirror' && extrasCount > 0` → mostrar modal de confirmación.
4. `run_robocopy(params, fileCount)` → escuchar `file_completed` (actualiza `remaining`/`lastFile`), `copy_error` (incrementa `errorCount` + flash), `copy_done` (→ `done`).
5. `cancel_robocopy()` desde el botón Cancelar.

*(Refinamiento vs. borrador previo: `error_partial` no es un estado aparte — el `copy_error` solo incrementa `errorCount` y dispara un flash rojo transitorio; la copia continúa.)*

### 3. CopyScreen + animación (Canvas, sin librerías)

Túnel hyperspace: ~200 partículas que viajan hacia un vanishing point central con motion blur simulado. Las partículas son palabras/fragmentos: nombres de archivo reales del evento `file_completed`, mezclados con strings genéricos (`0x`, `\\`, `.bak`, hashes cortos).

- **Normal:** colores fríos `#569CD6`, `#9CDCFE`, `#FFFFFF`.
- **Error parcial:** las partículas del archivo fallido aparecen en `#F44747` ~1.5s, luego retoman color normal.
- **Error fatal:** desaceleración, cambio gradual a `#F44747`, dispersión hacia afuera, fade del canvas.

Ciclo `requestAnimationFrame` con trail effect (`fillRect` semitransparente). ~200 partículas a 60fps es trivial; no requiere WebGL.

**Contador** (overlay inferior, monospace): `Archivos restantes: X de Y`. En `scanning_slow`: "Calculando archivos… Tu respaldo es grande, ten paciencia."

**Botón Cancelar** visible durante `copying`.

### 4. Transiciones

300–500ms, sin cortes. Flash rojo de 200ms en error parcial; desaceleración + interpolación de color en ~30 frames en error fatal.

### 5. ResultScreen

Layout (fondo `bg-zinc-900`, misma estética que CommandPreview):

```
[ícono estado]  Copia completada

robocopy  D:\Datos  E:\Respaldo  /E ...   ← display coloreado

Copiados:  1.228 archivos
Saltados:      6 archivos
Fallidos:      0 archivos
Duración:  4m 32s

[Exportar resultado]  [Volver]
```

Ícono por exit code: 0 → `○` gris · 1 → `✓` verde · 8 → `⚠` amarillo · 16 → `✕` rojo. Si hay fallidos, sección expandible "Ver archivos con error" (monospace, `#F44747`).

*(Nota: "Guardar perfil" queda fuera — es spec005. El ResultScreen solo tiene "Exportar resultado" y "Volver".)*

### 6. Export / Import `.plagg`

**Schema v0.1** (mínimo, espejo del formulario — sin `nombre` ni `flags`):

```json
{
  "origen": "D:\\Datos\\Geologia",
  "destino": "E:\\Respaldos\\Geologia",
  "modo": "incremental",
  "excluir": ["C:\\Temp", "D:\\Cache"]
}
```

**Export** (desde MainScreen antes de copiar y desde ResultScreen después):

```typescript
import { save } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

const path = await save({
  defaultPath: `respaldo-${carpetaOrigen}-${fecha}.plagg`,
  filters: [{ name: 'Plaggiarism Config', extensions: ['plagg'] }]
})
if (path) await invoke('write_text_file', { path, content: JSON.stringify(config, null, 2) })
```

El **nombre lo da el usuario en el diálogo "Guardar como…"** de Windows (el filename, no un campo interno). Nombre sugerido autogenerado: `respaldo-{origen}-{fecha}.plagg`.

**Import** (desde MainScreen, botón "Importar .plagg"):

```typescript
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

const path = await open({ filters: [{ name: 'Plaggiarism Config', extensions: ['plagg'] }] })
if (path) {
  const raw = await invoke<string>('read_text_file', { path })
  const config = JSON.parse(raw)
  // validar campos requeridos → poblar formulario (error inline si falta algo)
}
```

### 7. Modal de confirmación Mirror

Se activa cuando `modo === 'mirror'` y `extrasCount > 0` (archivos en destino que no están en origen y serían eliminados):

```
⚠️  Modo Espejo — Confirmación requerida

  Este modo eliminará 47 archivos en destino
  que no existen en el origen seleccionado.

  [Cancelar]   [Confirmar y continuar]
```

Si `extrasCount === 0`, no mostrar modal y proceder directo.

## Tasks

### Backend
- [x] Ampliar `build_scan_args(origen, destino, modo, excluir)` con `/MIR` para Mirror y `/XD` para exclusiones
- [x] Agregar `extrasCount` a `ScanResult` y extraerlo del resumen (columna "Extras", fila "Archivos")
- [x] Extraer `skipped` del resumen en `run_robocopy_core` (columna "Omitido", fila "Archivos")
- [x] Verificar cómputo de `copied` (no doble-descontar `failed`)
- [x] Comandos Rust `write_text_file` / `read_text_file` (std::fs) para export/import `.plagg`
- [x] Test Rust: scan Mirror devuelve `extrasCount` correcto

### Frontend
- [x] Crear `useCopyProcess.ts` con la máquina de estados + bucle `poll_scan`
- [x] Wiring: GO! → scan → (modal Mirror) → `run_robocopy` → eventos
- [x] Crear `CopyScreen.tsx` con canvas + animación hyperspace (estado normal)
- [x] Integrar nombres reales de `file_completed` como partículas
- [x] Contador "Archivos restantes: X de Y"
- [x] Mensaje de scan lento
- [x] Estado error parcial (flash rojo + contador de errores)
- [x] Estado error fatal (desaceleración → rojo → dispersión → mensaje)
- [x] Verificar transiciones suaves (sin cortes)
- [x] Botón Cancelar → `cancel_robocopy`
- [x] Crear `ResultScreen.tsx` con Copiados/Saltados/Fallidos/Duración
- [x] Ícono de estado según exit code
- [x] Sección expandible de archivos fallidos
- [x] Export `.plagg` (`save` + `writeTextFile` + schema mínimo)
- [x] Import `.plagg` (`open` + `readTextFile` + validación)
- [x] Modal Mirror (conteo de archivos a eliminar)

### Validación
- [x] Prueba e2e: MainScreen → GO! → animación → resultado
- [x] Prueba manual: export e import de `.plagg`
- [x] Prueba manual: modal Mirror con carpeta destino no vacía
- [x] Commit: `feat: animation engine, result screen and plagg config export`

---

*Spec 004 — Plaggiarism*
