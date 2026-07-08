# Spec 003 — Devlog de desarrollo

## Resumen

spec003 implementado en 4 commits. Scan funcional (`fileCount: 6080` confirmado
en prueba real). Copia real (`run_robocopy`) implementada pero sin probar
end-to-end aún.

## Bugs encontrados y corregidos

| # | Bug | Causa | Síntoma | Fix | Commit |
|---|---|---|---|---|---|
| 1 | `/NJS` suprimía el resumen | Añadí `/NJS` a `build_scan_args` | scan retorna `fileCount: 0` al instante, botón flicker | Removí `/NJS` | `d32d49e` |
| 2 | Parser no matcheaba output real | Spec mostraba ejemplo idealizado (solo dígitos). Robocopy real imprime `Archivos:` con dos puntos y etiqueta locale | `fileCount: 0` | Reescribir `parse_summary_line`: buscar `:`, parsear 6 enteros, usar fila 1 (Archivos) | `026e0a5` |
| 3 | Crash UTF-8 | `read_line` asume UTF-8; robocopy escribe CP1252. Rutas con acentos rompen el decode | `error de scan: stream did not contain valid UTF-8` | `read_until(b'\n')` + `encoding_rs::WINDOWS_1252` | `5ad5650` |

## Decisiones de diseño (divergencias del spec)

- **Cancelación:** `Arc<Mutex<Option<Child>>>` + `child.kill()` nativo (spec
  decía PID + crate externo). Menos dependencias, más portátil.
- **Scan lento:** añadido comando `poll_scan` (spec no definía cómo obtener
  resultado final tras timeout de 8s). `poll_scan` drena stdout del scan
  background hasta terminar.
- **`extrasCount`:** postergado a spec004 (spec004 lo necesita para modal
  Mirror). scan_robocopy retorna literal `{ fileCount, totalBytes, slow }`.

## Gap conocido

`totalBytes: 0` — con `/NFL /NDL` no hay líneas individuales de archivos; la
única fuente de bytes es la fila `Bytes:` del resumen, que usa formato
`19.1 k` (unidad de una letra con espacio) no reconocido por
`parse_size_bytes` (que espera `kb`/`mb`/`gb`). Postergado a spec004.

## Tasks de spec003 sin marcar

- `[ ]` Prueba manual: copia incremental real + eventos en consola (task 172)
- `[ ]` Prueba manual: cancel_robocopy detiene el proceso (task 173)

Flavio los marca al probar. Yo no toco `tasks.md`.

## Commits de spec003

```
5ad5650 fix: decode robocopy stdout as windows-1252 instead of utf-8
026e0a5 fix: parse robocopy summary rows with locale-aware labels (Archivos row)
d32d49e fix: remove /NJS flag from scan so summary line is parseable
b0cd8bb feat: rust robocopy core with subprocess, parser and tauri events
```

---

*Spec 003 — Plaggiarism*
