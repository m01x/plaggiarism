# Spec 007 — Portable distribution (empaquetado y distribución)

## Propósito

Cerrar el proyecto: empaquetar Plaggiarism como un ejecutable **portable auto-contenido**, definir cómo distribuirlo en la organización (servidor → clientes) sin firma de código, y sellar el release v0.1. Al cerrar este spec, el `.exe` queda listo para copiarse a un share o a los equipos y ejecutarse con un acceso directo.

## Scope

### Build portable
- Release build con `tauri build --no-bundle` → `src-tauri/target/release/plaggiarism.exe` auto-contenido (~5 MB, frontend embebido).
- (Opcional) Instalador NSIS/MSI con `tauri build` (targets por defecto) para una distribución formal, si se quiere.
- Verificar el `.exe` en una máquina Windows **limpia** (sin Node/Rust) como smoke test.

### Distribución (sin firma de código)
- **SmartScreen / Mark of the Web**: el exe sin firmar puede mostrar "Windows protegió su PC" si fue descargado por navegador/correo. Solución sin costo:
  1. Distribuir por **copia de red / robocopy** (no agrega Mark of the Web) → sin aviso.
  2. Si se descarga por navegador: **`Unblock-File`** (o botón "Desbloquear" en Propiedades) para quitar el MotW.
  3. Alternativa aceptable: el usuario pulsa "Ejecutar de todas formas".
- **AppLocker / GPO** (para entornos con bloqueo de ejecución): permitir el exe con una **regla por hash o por ruta** vía directiva de grupo (solo en Windows Pro/Enterprise). No requiere certificado.
- **WebView2**: preinstalado en Windows 11 y Windows 10 actualizado. Para Win7/8 o máquinas "peladas", instalar el Evergreen Bootstrapper (o usar el instalador, que lo descarga solo).

### Cierre
- Limpiar dependencia `regex` declarada y sin uso en `src-tauri/Cargo.toml`.
- `README.md`: sección de build y de distribución portable.
- Tag y commit de release `v0.1.0` (versión ya en `0.1.0`).

## Fuera de scope

- Firma de código con certificado (decisión tomada: **no**, por costo).
- Auto-updater.
- Instalador MSI personalizado / soporte multi-idioma del instalador.
- Icono/identidad definitiva plagg.io (opcional: si hay assets, se cambian los `icons/*` antes del release).

## Contrato de API

Sin cambios. Este spec no toca comandos ni eventos; solo build y documentación.

## Plan técnico

1. **Build**: confirmar `productName: "Plaggiarism"` e `identifier: "io.plagg.plaggiarism"` (ya correctos). Ejecutar `pnpm tauri build --no-bundle` (o `--bundles none`) y verificar el binario en `target/release/`.
2. **Smoke test**: copiar `plaggiarism.exe` a una máquina limpia, ejecutar, hacer un GO! real y validar scan → copia → resultado.
3. **Distribución**: documentar en README los tres caminos (share + acceso directo, copia local, o instalador) y las notas SmartScreen/AppLocker/WebView2.
4. **Limpieza**: quitar `regex` de `Cargo.toml` (verificado: no hay `use regex` ni `Regex::` en el código).

## Tasks

### Build
- [ ] Release build `tauri build --no-bundle` → `plaggiarism.exe` auto-contenido
- [ ] Smoke test del exe en máquina Windows limpia (scan → copia → resultado)

### Distribución
- [ ] Documentar SmartScreen / Mark of the Web y `Unblock-File`
- [ ] Documentar whitelist AppLocker/GPO (regla hash o path) para entornos bloqueados
- [ ] Documentar requisito WebView2 (preinstalado Win10/11) + Evergreen Bootstrapper

### Cierre
- [ ] Quitar dependencia `regex` sin uso
- [ ] `README.md`: sección "Build" y "Distribución portable"
- [ ] Commit: `chore: portable build and distribution docs`
- [ ] Tag `v0.1.0` y commit de release

## Decisiones técnicas (registro para no perder)

- **Firma**: sin costo, vía distribución por red (sin Mark of the Web) + `Unblock-File` + AppLocker/GPO cuando aplique. Sin certificado.
- **Stateless**: sin `%APPDATA%`, sin historial ni perfiles; config vía `.plagg`.
- **WebView2** es la única dependencia externa del exe; preinstalado en Win10/11.
- **Versión**: el release sale como `v0.1.0` (coincide con `0.1.0` actual en `Cargo.toml`/`tauri.conf.json`/`package.json`).

## Roadmap (cierre)

1. ✅ Spec 006 — Space detection
2. ✅ Spec 007 — Portable distribution (este spec)
3. ⏳ Release v0.1 — tag + merge a main

---

*Spec 007 — Plaggiarism*
