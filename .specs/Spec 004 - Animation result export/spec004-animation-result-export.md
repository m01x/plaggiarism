# Spec 004 — Animation Engine + Result Screen + Export .plagg

## Propósito

Implementar la capa visual del proceso de copia: la animación de túnel hyperspace que consume los eventos del spec003 en tiempo real, la pantalla de resultado final con la misma estética que Command Preview, y el export/import de configuraciones `.plagg`. Al cerrar este spec, Plaggiarism es un producto funcional de punta a punta — desde la pantalla principal hasta el resultado final, con la identidad visual completa.

---

## Scope

- Pantalla de ejecución con animación túnel hyperspace (Canvas API)
- Estados de la animación: normal → error parcial → error fatal
- Máquina de estados que consume eventos de Tauri (`file_completed`, `copy_error`, `copy_done`)
- Contador de archivos restantes visible durante la copia
- Transiciones animadas entre todos los estados (sin cortes abruptos)
- Pantalla de resultado (VS Code Dark+ theme)
- Botón y lógica de export a archivo `.plagg`
- Import de archivo `.plagg` desde la pantalla principal
- Modal de advertencia para modo Mirror (con conteo de archivos a eliminar)

## Fuera de scope

- Historial persistente de trabajos (spec005)
- Perfiles guardados internamente (spec005)
- Tooltips en flags del Command Preview (spec005)

---

## Plan técnico

### Máquina de estados del proceso

```typescript
type CopyState =
  | { status: 'idle' }
  | { status: 'scanning' }
  | { status: 'scanning_slow' }            // scan supera 8 segundos
  | { status: 'copying'; remaining: number; total: number; lastFile: string }
  | { status: 'error_partial'; errorCount: number; remaining: number }
  | { status: 'error_fatal'; message: string }
  | { status: 'done'; result: RobocopyResult }
```

Esta máquina vive en un hook `useCopyProcess.ts` que:
1. Llama `scan_robocopy` al presionar GO!
2. Transiciona a `scanning` → `copying` cuando el scan termina
3. Escucha eventos de Tauri y actualiza el estado
4. Maneja transiciones de error

### Pantalla de ejecución

Un componente `CopyScreen.tsx` que recibe el estado actual y renderiza la animación correspondiente. La transición entre `MainScreen` y `CopyScreen` es una fade rápida (300ms).

### Animación — túnel hyperspace

Implementada con **Canvas API** (`<canvas>` nativo). No requiere librería externa — esto mantiene el ejecutable liviano.

**Concepto visual:**
- ~200 partículas que parten desde posiciones aleatorias en la pantalla y viajan hacia el centro de perspectiva (vanishing point)
- Cada partícula es una palabra o fragmento: nombres de archivo que van llegando del evento `file_completed`, mezclados con strings genéricos ("0x", "\\\\", ".bak", fechas, hashes cortos)
- Efecto de velocidad de la luz: las partículas se alargan en su dirección de movimiento (motion blur simulado con `lineTo` de longitud variable según velocidad)
- El centro de perspectiva pulsa levemente (oscilación suave de ±5px) para dar sensación de movimiento orgánico

**Ciclo de animación (requestAnimationFrame):**
1. Limpiar canvas con `fillRect` semitransparente (`rgba(0,0,0,0.15)`) para trail effect
2. Por cada partícula: avanzar hacia el centro, aumentar velocidad con la distancia (simula aceleración perspectiva), actualizar posición
3. Si la partícula llega al centro: resetear a posición aleatoria en el borde
4. Renderizar el texto con `fillText` en el color del estado actual

**Paleta de colores por estado:**
- Normal: `#569CD6`, `#9CDCFE`, `#FFFFFF` (azul/cyan/blanco — coherente con VS Code)
- Error parcial: las partículas del archivo fallido aparecen en `#F44747` (rojo) durante ~1.5 segundos, luego el sistema retoma el color normal
- Error fatal: todas las partículas cambian gradualmente a `#F44747`, la animación desacelera y las partículas se dispersan hacia afuera (invertir dirección del vanishing point)

**Rendimiento:** Canvas a 60fps con ~200 partículas es trivial para cualquier hardware moderno. No requiere WebGL.

### Contador de archivos

Overlay sobre el canvas, posicionado abajo al centro:

```
Archivos restantes: 847 de 1.234
```

Fuente monospace, `text-zinc-300`. Se actualiza con cada evento `file_completed`. En estado `scanning_slow`, reemplazar por:

```
Calculando archivos... Tu respaldo es grande, por favor ten paciencia.
```

### Transiciones de estado

Todas las transiciones tienen duración de 300–500ms. Implementadas con CSS transitions sobre el opacity/transform del overlay de texto, y con cambio gradual de color en Canvas (interpolación de hex a hex en ~30 frames).

**Estado normal → error parcial:**
- El canvas no se interrumpe — sigue corriendo
- Un flash rojo de 200ms en el borde del canvas
- El contador muestra `(X errores)` en rojo junto al conteo normal

**Estado normal/error_parcial → error_fatal:**
- Las partículas desaceleran durante 1 segundo (ease-out en velocidad)
- Cambio de color a rojo en 30 frames
- Las partículas invierten dirección (van desde el centro hacia afuera)
- El canvas hace fade a opacity 0.3 al fondo
- El mensaje de error aparece en el centro con fade-in (400ms)

### Pantalla de resultado

Componente `ResultScreen.tsx`. Transición desde `CopyScreen`: fade de 400ms.

Layout (fondo `bg-zinc-900`, misma estética que CommandPreview):

```
┌────────────────────────────────────────────┐
│  [ícono estado]  Copia completada          │
│                                            │
│  robocopy  D:\Datos  E:\Respaldo  /E ...   │  ← mismo display coloreado que CommandPreview
│                                            │
│  Copiados:    1.228 archivos               │
│  Saltados:       6 archivos                │
│  Fallidos:        0 archivos               │
│  Duración:    4m 32s                       │
│                                            │
│  [Exportar resultado]  [Volver]  [Guardar perfil] │
└────────────────────────────────────────────┘
```

Si hubo archivos fallidos, mostrar una sección expandible "Ver archivos con error" con la lista formateada — misma fuente monospace, color `#F44747`.

El ícono de estado sigue la tabla de exit codes:
- 0 → `○` gris ("Destino ya estaba actualizado")
- 1 → `✓` verde
- 8 → `⚠` amarillo
- 16 → `✕` rojo

### Export / Import `.plagg`

**Export** (disponible desde MainScreen antes de copiar y desde ResultScreen después):

```typescript
import { save } from '@tauri-apps/plugin-dialog'
import { writeTextFile } from '@tauri-apps/plugin-fs'

const path = await save({
  defaultPath: `${nombreSugerido}.plagg`,
  filters: [{ name: 'Plaggiarism Config', extensions: ['plagg'] }]
})

if (path) {
  await writeTextFile(path, JSON.stringify(config, null, 2))
}
```

El nombre sugerido se genera automáticamente: `respaldo-{carpeta_origen}-{fecha}.plagg`

**Import** (desde MainScreen, botón "Importar .plagg"):

```typescript
import { open } from '@tauri-apps/plugin-dialog'
import { readTextFile } from '@tauri-apps/plugin-fs'

const path = await open({
  filters: [{ name: 'Plaggiarism Config', extensions: ['plagg'] }]
})

if (path) {
  const raw = await readTextFile(path as string)
  const config = JSON.parse(raw)
  // poblar el formulario con los valores del config
}
```

Validar el JSON importado antes de poblar el formulario — si falta algún campo requerido, mostrar un error inline, no un crash.

### Modal de advertencia Mirror

Se activa cuando: modo = Mirror Y el scan detectó archivos en destino que no están en origen.

El scan `/L` ya corre antes de la copia. Del resultado del scan, el campo `extrasCount` (archivos "Extras" en el resumen de robocopy) indica cuántos archivos del destino no existen en origen y serían eliminados.

Si `extrasCount > 0`:

```
⚠️  Modo Espejo — Confirmación requerida

  Este modo eliminará 47 archivos en destino
  que no existen en el origen seleccionado.

  [Ver lista]    [Cancelar]    [Confirmar y continuar]
```

Si `extrasCount === 0`: no mostrar modal, proceder directamente.

---

## Tasks

- [ ] Crear hook `useCopyProcess.ts` con la máquina de estados completa
- [ ] Wiring: GO! → scan → (resultado del scan) → transición a CopyScreen
- [ ] Crear `CopyScreen.tsx` como contenedor de animación + overlay
- [ ] Implementar animación hyperspace base en Canvas (partículas, vanishing point, motion blur)
- [ ] Integrar nombres de archivos reales del evento `file_completed` como texto de partículas
- [ ] Implementar estado normal de la animación (colores azul/cyan/blanco)
- [ ] Implementar estado error parcial (flash rojo, retoma velocidad)
- [ ] Implementar estado error fatal (desaceleración, cambio a rojo, dispersión)
- [ ] Implementar contador "Archivos restantes: X de Y"
- [ ] Implementar mensaje de scan lento
- [ ] Verificar que todas las transiciones son suaves (sin cortes)
- [ ] Crear `ResultScreen.tsx` con layout y datos reales de `RobocopyResult`
- [ ] Implementar sección expandible de archivos fallidos
- [ ] Implementar ícono de estado según exit code
- [ ] Implementar export `.plagg` con `plugin-dialog` y `plugin-fs`
- [ ] Implementar import `.plagg` con validación de campos
- [ ] Agregar botón "Importar .plagg" en `MainScreen`
- [ ] Implementar detección de `extrasCount` en resultado del scan
- [ ] Implementar modal de advertencia Mirror con lista de archivos a eliminar
- [ ] Prueba manual end-to-end: MainScreen → GO! → animación → resultado
- [ ] Prueba manual: export e import de `.plagg`
- [ ] Prueba manual: modal Mirror con carpeta destino no vacía
- [ ] Commit: `feat: animation engine, result screen and plagg config export`

---

*Spec 004 — Plaggiarism*
