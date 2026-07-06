# Spec 003 — Rust Robocopy Core

## Propósito

Implementar toda la capa Rust de Plaggiarism: los Tauri commands que invocan robocopy como proceso hijo, el scanner previo (`/L`), el parser de stdout línea por línea con emisión de eventos al frontend, y el manejo de exit codes. Al cerrar este spec, la aplicación ejecuta robocopy real y el frontend recibe eventos en tiempo real. Los stubs del spec002 son reemplazados por las implementaciones reales.

---

## Scope

- Definición del contrato de API interna (Tauri commands)
- Comando `scan_robocopy`: corre robocopy `/L`, devuelve conteo de archivos, con timeout de 8 segundos
- Comando `run_robocopy`: corre la copia real, emite eventos por archivo completado
- Comando `cancel_robocopy`: termina el proceso hijo limpiamente
- Comando `validate_paths`: verifica que origen y destino existen y son accesibles
- Parser de stdout: extrae archivos completados línea por línea
- Manejo de exit codes (0, 1, 8, 16)
- Actualización del frontend: reemplazar stubs de `src/lib/robocopy.ts` con llamadas reales a `invoke()`

## Fuera de scope

- Animación de copia (spec004)
- Historial y perfiles (spec005)
- Modal de confirmación Mirror — la lógica de detección de archivos a eliminar entra acá, pero la UI del modal va en spec004

---

## Plan técnico

### Contrato de API interna

Este contrato se define antes de implementar. Es el único archivo que coordina frontend y backend — no se modifica sin aprobación del developer.

```typescript
// .specs/tauri-commands.md (referencia)

invoke('validate_paths', { origen: string, destino: string })
  → Promise<{ origenOk: boolean, destinoOk: boolean }>

invoke('scan_robocopy', { origen: string, destino: string, modo: string, excluir: string[] })
  → Promise<{ fileCount: number, totalBytes: number, slow: boolean }>
  // slow: true si el scan superó 8 segundos (frontend muestra mensaje de paciencia)

invoke('run_robocopy', { origen: string, destino: string, modo: string, excluir: string[] })
  → Promise<RobocopyResult>

invoke('cancel_robocopy')
  → Promise<void>
```

Eventos emitidos por Rust al frontend durante `run_robocopy`:
```typescript
'file_completed'  → { remaining: number, fileName: string }
'copy_error'      → { fileName: string, errorCode: number }
'copy_done'       → RobocopyResult
```

### Dependencias Cargo.toml

```toml
[dependencies]
tauri = { version = "2", features = ["shell-open"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Comando `validate_paths`

Verificación sincrónica simple:
- `std::path::Path::new(origen).exists()` y `is_dir()`
- No requiere robocopy

### Comando `scan_robocopy`

Construye el comando:
```
robocopy <origen> <destino> /L /E /NFL /NDL /NJH
```
- `/L`: list-only, no copia
- `/NFL /NDL /NJH`: reduce el noise del output para facilitar el parse

Corre el proceso con timeout de 8 segundos usando `tokio::time::timeout`. Si se agota, retorna `{ slow: true }` con el conteo parcial que haya acumulado hasta ese momento — no cancela el proceso, sigue corriendo en background y el frontend muestra el mensaje de paciencia.

El total de archivos lo extrae del resumen final que robocopy imprime al terminar:
```
   Total    Copied   Skipped  Mismatch    FAILED    Extras
    1234       456       778         0         0         0
```
Parsear esa línea con un regex simple: `r"^\s+(\d+)\s+(\d+)\s+(\d+)"`.

### Comando `run_robocopy`

Construye el comando según modo:
- Incremental: `robocopy <origen> <destino> /E /W:1 /R:1`
- Mirror: `robocopy <origen> <destino> /MIR /W:1 /R:1`

Si hay rutas excluidas: agrega `/XD <ruta1> <ruta2> ...`

**No usar el flag `/NP`** — necesitamos el porcentaje por archivo para detectar cuándo un archivo terminó. Cuando robocopy imprime `100%` seguido del nombre del archivo, ese archivo está completado.

Patrón de línea completada:
```
100%	New File		   1.23 mb	nombre_archivo.ext
100%	newer			   512 kb	otro_archivo.docx
```

El parser busca líneas que comiencen con `100%` y extrae el nombre del archivo. Cada match emite el evento `file_completed` con el contador decrementado.

Manejo del proceso hijo:
```rust
let mut child = std::process::Command::new("robocopy")
    .args(&args)
    .stdout(std::process::Stdio::piped())
    .spawn()?;
```

Guardar el PID en un `Mutex<Option<u32>>` global para poder matarlo con `cancel_robocopy`.

### Exit codes

```rust
match exit_code {
    0 => RobocopyStatus::NothingToDo,
    1 => RobocopyStatus::Success,
    2 => RobocopyStatus::ExtraFiles,      // archivos extra en destino (solo info)
    3 => RobocopyStatus::SuccessWithExtra,
    8 => RobocopyStatus::SomeFailed,
    16 => RobocopyStatus::FatalError,
    _ => RobocopyStatus::Unknown(exit_code),
}
```

`RobocopyResult` incluye: status, archivos copiados, saltados, fallidos, lista de archivos fallidos (si los hay), duración en segundos.

### Frontend: reemplazar stubs

En `src/lib/robocopy.ts`, reemplazar los stubs del spec002 con llamadas reales:

```typescript
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export async function runRobocopy(params: RobocopyParams) {
  return invoke('run_robocopy', params)
}

export function onFileCompleted(cb: (data: FileCompletedEvent) => void) {
  return listen('file_completed', (event) => cb(event.payload))
}
```

---

## Tasks

- [ ] Documentar el contrato de API interna en `.specs/tauri-commands.md`
- [ ] Agregar dependencias a `Cargo.toml` (tokio, serde, serde_json)
- [ ] Implementar struct `RobocopyParams` y `RobocopyResult` con serde
- [ ] Implementar `validate_paths` command
- [ ] Implementar `scan_robocopy` command con timeout de 8 segundos
- [ ] Verificar que el parser del resumen final extrae `fileCount` correctamente
- [ ] Implementar `run_robocopy` command con construcción correcta de argumentos
- [ ] Implementar parser de stdout línea por línea (detección de `100%`)
- [ ] Implementar emisión de evento `file_completed` con contador
- [ ] Implementar emisión de evento `copy_error` en archivos fallidos
- [ ] Implementar emisión de evento `copy_done` con `RobocopyResult`
- [ ] Implementar `cancel_robocopy` command con kill del proceso hijo
- [ ] Mapear todos los exit codes a `RobocopyStatus`
- [ ] Reemplazar stubs en `src/lib/robocopy.ts` con llamadas reales a `invoke()`
- [ ] Exponer listeners de eventos (`onFileCompleted`, `onCopyError`, `onCopyDone`)
- [ ] Prueba manual: correr una copia incremental real y verificar eventos en consola
- [ ] Prueba manual: verificar que cancel_robocopy detiene el proceso
- [ ] Commit: `feat: rust robocopy core with subprocess, parser and tauri events`

---

*Spec 003 — Plaggiarism*
