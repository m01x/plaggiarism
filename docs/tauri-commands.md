# Contrato de API interna — Tauri Commands

> Referencia operativa coordinando frontend y backend.
> Fuente de verdad: `.specs/Spec 003 - Rust robocopy core/spec003-rust-robocopy-core.md`.

## Comandos (`invoke()`)

```typescript
invoke('validate_paths', { origen: string, destino: string })
  → Promise<{ origenOk: boolean, destinoOk: boolean }>

invoke('scan_robocopy', { origen: string, destino: string, modo: string, excluir: string[] })
  → Promise<{ fileCount: number, totalBytes: number, slow: boolean }>
  // slow === true  → scan superó 8s, proceso sigue en background.
  //                  El frontend debe llamar poll_scan() hasta obtener
  //                  resultado final (o None mientras siga corriendo).

invoke('poll_scan')
  → Promise<{ fileCount: number, totalBytes: number, slow: boolean } | null>
  // null  → no hay scan en curso o ya fue consumido.
  // slow:true con conteo parcial  → sigue corriendo.
  // slow:false  → scan terminó, resultado final.

invoke('run_robocopy', { origen: string, destino: string, modo: string, excluir: string[] })
  → Promise<RobocopyResult>

invoke('cancel_robocopy')
  → Promise<void>
```

## Eventos (emitidos por Rust, escuchados con `listen()`)

```typescript
'file_completed'  → { remaining: number, fileName: string }
'copy_error'      → { fileName: string, errorCode: number }
'copy_done'       → RobocopyResult
```

## Structs compartidos (serializados camelCase)

```typescript
interface FailedFile {
  fileName: string
  errorCode: number
}

interface RobocopyResult {
  status: RobocopyStatus
  copied: number
  skipped: number
  failed: number
  failedFiles: FailedFile[]
  durationSecs: number
}

type RobocopyStatus =
  | 'NothingToDo'        // exit 0
  | 'Success'            // exit 1
  | 'ExtraFiles'         // exit 2
  | 'SuccessWithExtra'   // exit 3
  | 'SomeFailed'         // exit 8
  | 'FatalError'         // exit 16
  | { Unknown: number }  // otro
```

## Notas de implementación

- **Cancelación:** Rust guarda `Arc<Mutex<Option<Child>>>` en estado global. `cancel_robocopy` llama `child.kill()` + `child.wait()` para reap.
- **Scan lento:** `tokio::time::timeout(8s, ...)`. Si se agota, el `Child` y su `BufReader` se guardan en `ScanState` para que `poll_scan` continúe drenando stdout.
- **Parser de archivo completado:** lectura por `\r`; el nombre del archivo se rastrea con `parse_file_info` (`current_file`) y la finalización se detecta en el segmento `100%`.
- **Atribución de errores:** la línea `ERROR <code>` se asocia al `current_file` en curso (no al último completado), con `error_code` real parseado y deduplicación por reintento `/R:1`.
- **Parser de resumen final:** regex sobre la línea `Total  Copied  Skipped  Mismatch  FAILED  Extras`; `skipped` y `failed` (columna ERROR) se leen de la fila "Archivos".
- **`/NP` prohibido** — necesitamos el porcentaje por archivo para detectar completados.