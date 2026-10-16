# Spec 005 — Hardening y saneado (correcciones del review de spec004)

## Propósito

Dejar el sistema verificado y consistente antes de agregar features nuevas (spec006). Corrige los hallazgos del review de spec004: atribución incorrecta de archivos fallidos, conteo de `failed` no autoritativo, `error_code` hardcodeado, `validate_paths` sin cablear y código muerto. Al cerrar este spec, el flujo punta a punta reporta **exactamente** los archivos fallidos y valida rutas antes de `GO!`.

## Scope

### Backend (`src-tauri/`)

- **Atribución correcta de archivos fallidos** en `run_robocopy_core` (`robocopy.rs`): usar `current_file` (archivo en curso, seteado por `parse_file_info`) en lugar de `last_file_for_error` (archivo **completado** anterior). Eliminar `last_file_for_error`.
- **`failed` autoritativo**: leer la columna `ERROR` del resumen (fila "Archivos", `nums[4]`) como conteo oficial. Fallback al conteo de líneas de error detectadas si robocopy falló fatalmente sin emitir resumen.
- **`failed_files` deduplicado**: con `/R:1` robocopy puede emitir la misma línea de error una vez por reintento; no duplicar la misma entrada ni emitir `copy_error` doble.
- **`error_code` real**: helper `parse_error_code(seg) -> Option<i32>` que extrae el número de `ERROR <code> (0x…)`; emitirlo en `CopyErrorEvent.error_code` (hoy hardcodeado a `8`).
- **`RobocopyResult.failed_files`** pasa de `Vec<String>` a `Vec<FailedFile>` donde `FailedFile = { fileName, errorCode }`.
- **Código muerto / limpieza**: quitar `parse_file_completed`, `parse_size_bytes`, el campo `total_bytes` de `ScanCounters` y su acumulación (diferido a v0.2). `total_bytes` se mantiene en el contrato (`ScanResult`) devolviendo `0` con comentario explícito.

### Frontend (`src/`)

- **Cablear `validate_paths`** antes de `GO!` en `MainScreen.handleGo`: invocar `validatePaths(origen, destino)` y, si `origenOk`/`destinoOk` es `false`, mostrar error inline y **no** lanzar la copia.
- **Mostrar `errorCode` real** en la lista de fallidos de `ResultScreen` (`{fileName} (código {errorCode})`).

### Tests

- **Unit test** de `parse_error_code` (formato real `ERROR 32 (0x00000020) …`).
- **e2e Windows de archivo fallido**: bloquear un archivo de origen con handle abierto sin compartir (`std::os::windows::fs::OpenOptionsExt::share_mode(0)`) y verificar `copied`, `failed` y `failedFiles` correctos. Este es el test que faltaba y que dejó pasar el bug de atribución.
- Re-ejecutar los e2e existentes (incremental, cancel, progreso, mirror-extras) para confirmar que no hubo regresiones.

## Fuera de scope

- Historial persistente de trabajos (spec006).
- Perfiles guardados internamente (spec006).
- Detección de espacio insuficiente en destino (`totalBytes`) (v0.2).
- Tooltips en flags del Command Preview: **ya están implementados** (ver Notas).

## Contrato de API (forma sin cambios; semántica corregida)

| Invocación | Resultado |
|---|---|
| `validate_paths({ origen, destino })` | `{ origenOk, destinoOk }` *(ahora se invoca antes de GO)* |
| `scan_robocopy({ params })` | `{ fileCount, totalBytes, slow, extrasCount }` |
| `poll_scan()` | `{ fileCount, totalBytes, slow } \| null` |
| `run_robocopy({ params, totalFiles })` | `RobocopyResult` |
| `cancel_robocopy()` | `void` |

Eventos emitidos por Rust durante `run_robocopy`:

```
'file_completed'  → { remaining: number, fileName: string }
'copy_error'      → { fileName: string, errorCode: number }   // errorCode real
'copy_done'       → RobocopyResult
```

`RobocopyResult = { status, copied, skipped, failed, failedFiles, durationSecs }` donde `failedFiles: FailedFile[]` con `FailedFile = { fileName, errorCode }`. `failed` pasa a ser el valor de la columna `ERROR` del resumen (fallback a conteo de líneas de error en fallo fatal).

## Plan técnico

### 1. `run_robocopy_core` (`robocopy.rs`)

- En el branch de error, reemplazar `last_file_for_error` por `current_file.clone()`.
- Capturar `failed_summary = nums[4]` en `summary_rows_seen == 2` (junto a `skipped`).
- `failed = failed_summary` si `summary_rows_seen >= 2`; si no (fallo fatal sin resumen), `failed = failed_files.len()`.
- `failed_files` deduplicado: no empujar si `file_name` coincide con el último.
- Helper `parse_error_code(seg: &str) -> Option<i32>`.

### 2. `MainScreen.handleGo` (`MainScreen.tsx`)

- `const { origenOk, destinoOk } = await validatePaths(origen, destino);`
- Si falla, setear un mensaje de error inline (estado `goError`) y `return` sin lanzar.

### 3. Tests

- `parse_error_code` unitario (síncrono, sin robocopy).
- e2e `e2e_failed_file_reports_error`: 2 archivos sanos + 1 bloqueado → `copied == 2`, `failed == 1`, `failedFiles` contiene `locked.txt`.

## Tasks

### Backend
- [x] Corregir atribución de fallidos: `current_file` en lugar de `last_file_for_error`
- [x] `failed` desde columna `ERROR` del resumen (`nums[4]`) con fallback
- [x] `failed_files` deduplicado
- [x] Parsear `error_code` real y emitirlo en `copy_error`
- [x] `failed_files: Vec<FailedFile>` (`{ fileName, errorCode }`)
- [x] Extraer `parse_error_code` como helper testeable
- [x] Limpieza de código muerto (`parse_file_completed`, `parse_size_bytes`, `total_bytes` en `ScanCounters`)

### Frontend
- [x] Cablear `validate_paths` antes de `GO!` con error inline
- [x] Mostrar `errorCode` en la lista de fallidos

### Tests / Validación
- [x] Unit test `parse_error_code`
- [x] e2e Windows: archivo fallido → `copied`, `failed` y `failedFiles` correctos
- [x] Re-ejecutar suite completa (incremental, cancel, progreso, mirror-extras)
- [x] Prueba manual: carpeta con un archivo bloqueado → verificar pantalla de resultado
- [x] Commit: `fix: correct failed-file attribution and count, wire path validation`

## Notas para el gate (decisiones de Flavio)

1. **Tooltips ya existen.** `CommandPreview.tsx` ya implementa tooltips en flags (`title` + `flagTip`), aunque `PLAGGIARISM.md` los lista como v0.2 pendientes. Sugiero quitarlos del scope de spec006.
2. **Re-scope de spec006.** Tras este saneado, spec006 (config-profiles) queda con: historial persistente + perfiles internos + detección de espacio. Sin tooltips.
3. **`totalBytes`.** Se mantiene en el contrato devolviendo `0` de forma explícita y documentada; se retoma cuando exista detección de espacio.

---

*Spec 005 — Plaggiarism*
