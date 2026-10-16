# Spec 006 — Space detection (detección de espacio en destino)

## Propósito

Cerrar el último feature pendiente del v0.2: advertir (y bloquear) antes de `GO!` si el destino no tiene espacio suficiente para la copia. La app queda **stateless** (sin historial ni perfiles internos); "guardar configuración" se cubre con el export/import `.plagg` ya existente. Con esto y el packaging portable (Spec 007), el proyecto queda listo para distribuirse.

## Scope

### Backend (`src-tauri/`)
- Restaurar `total_bytes` real en el scan: parsear la fila "Bytes" del resumen de robocopy (locale-aware: `k`/`m`/`g` o `kb`/`mb`/`gb`), columna "Copiado" (bytes a copiar).
- Nuevo comando `disk_free_space(path) -> u64`: bytes libres del volumen que contiene `path` (WinAPI `GetDiskFreeSpaceExW` vía crate `windows`/`windows-sys`, o `sysinfo`).
- `ScanResult.total_bytes` deja de devolver `0` y vuelve a tener valor real.

### Frontend (`src/`)
- Chequeo previo a `GO!`: comparar `scan.totalBytes` con el espacio libre del destino; si `totalBytes > freeSpace` (con margen), bloquear y mostrar aviso.

### Tests
- Unit test del parser de "Bytes" (locale ES: `12.3 m`, `512 k`, `2.5 g`).
- Test del comando `disk_free_space` (o mock).

## Fuera de scope

- Historial persistente (decisión: app **stateless**).
- Perfiles internos (decisión: `.plagg` ya cubre guardar/cargar config).
- Historial de sesión en memoria (no necesario por ahora; el CMD tampoco guarda nada).
- Firma de código con certificado (decisión: alternativa sin costo, ver Notas).

## Contrato de API

| Invocación | Resultado |
|---|---|
| `scan_robocopy({ params })` | `{ fileCount, totalBytes, slow, extrasCount }` *(totalBytes real)* |
| `disk_free_space({ path })` | `number` (bytes libres) |

## Plan técnico

1. **Parser "Bytes"** (`robocopy.rs`): parsear la fila `Bytes:` del resumen. La columna "Copiado" es el total de bytes a copiar. Aceptar unidades ES (`k`, `m`, `g`) y EN (`kb`, `mb`, `gb`), con decimales.
2. **`disk_free_space`** (`commands.rs`): obtener el volumen de la ruta destino (raíz de unidad o `\\server\share`) y llamar `GetDiskFreeSpaceExW`.
3. **Chequeo pre-GO** (`useCopyProcess.ts`): tras `scan_robocopy`, llamar `disk_free_space(destino)` y comparar. Si no alcanza, estado de error previo a la copia.

## Tasks

### Backend
- [ ] Parser de la fila "Bytes" (locale-aware) → `total_bytes` real
- [ ] Comando `disk_free_space(path)` (WinAPI)
- [ ] `ScanResult.total_bytes` con valor real

### Frontend
- [ ] Chequeo de espacio antes de `GO!` (bloquear + aviso)

### Tests / Validación
- [ ] Unit test del parser "Bytes"
- [ ] Test `disk_free_space`
- [ ] Prueba manual: destino casi lleno → aviso; destino con espacio → pasa
- [ ] Commit: `feat: disk space check before copy`

## Decisiones técnicas y roadmap (registro para no perder)

- **Firma de código**: alternativa **sin costo** — lista blanca por GPO/AppLocker en las máquinas de la organización (no certificado de firma). Detalle en Spec 007.
- **Stateless**: sin `%APPDATA%`, sin historial ni perfiles internos; config se exporta/importa vía `.plagg`.
- **Roadmap**:
  1. Spec 006 — Space detection (este spec).
  2. Spec 007 — Portable & distribución: exe auto-contenido, nota WebView2, whitelist GPO/AppLocker, despliegue al servidor.
  3. Release v0.1 — merge, tag, commit.

---

*Spec 006 — Plaggiarism*
