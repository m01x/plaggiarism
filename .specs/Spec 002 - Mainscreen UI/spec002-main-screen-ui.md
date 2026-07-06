# Spec 002 — Main Screen UI

## Propósito

Implementar la pantalla principal completa de Plaggiarism como una UI funcional en React. Al cerrar este spec, el developer puede interactuar con todos los controles (pickers, modo, exclusiones, preview) y el botón GO! responde correctamente a las validaciones. Los Tauri commands no existen aún — se mockean con stubs en frontend. La pantalla debe verse como el producto final, no como un prototipo.

---

## Scope

- File pickers para Origen y Destino (con diálogo real de Tauri)
- Indicadores visuales de estado (verde/rojo) junto a cada picker
- Selector de modo: radio buttons Incremental / Mirror con descripciones
- Sección de exclusión de rutas (input + botón agregar + lista)
- Command Preview con syntax highlighting estilo VS Code Dark+
- Botón GO! con lógica de validación y estados habilitado/deshabilitado
- Stubs de los Tauri commands necesarios (para desbloquear el siguiente spec)
- Estilos y layout final — no wireframe

## Fuera de scope

- Ejecución real de robocopy
- Animación de copia
- Historial y perfiles
- Export/Import `.plagg`
- Modal de confirmación Mirror (entra en spec posterior)

---

## Plan técnico

### Estructura de componentes

```
src/
├── screens/
│   └── MainScreen.tsx          # Pantalla principal (orquesta todo)
├── components/
│   ├── PathPicker.tsx           # Picker reutilizable (origen y destino)
│   ├── ModeSelector.tsx         # Radio buttons Incremental / Mirror
│   ├── ExcludePathsList.tsx     # Sección de rutas excluidas
│   ├── CommandPreview.tsx       # Caja de preview estilo VS Code
│   └── GoButton.tsx             # Botón GO! con estado de validación
├── hooks/
│   └── useMainForm.ts           # Estado global del formulario + validaciones
└── lib/
    └── robocopy.ts              # Stubs de los Tauri commands
```

### Estado del formulario (`useMainForm.ts`)

Un hook centralizado que mantiene:

```typescript
interface FormState {
  origen: string
  destino: string
  modo: 'incremental' | 'mirror' | null
  excluir: string[]
}
```

Y expone:
- `isValid`: boolean — true solo si origen, destino y modo están completos
- `commandString`: string — el comando robocopy generado en tiempo real
- Handlers para cada campo

### PathPicker

Usa `@tauri-apps/plugin-dialog` para abrir el diálogo de selección de carpeta:

```typescript
import { open } from '@tauri-apps/plugin-dialog'

const folder = await open({ directory: true, multiple: false })
```

Indicador visual:
- Sin valor → círculo rojo (`bg-red-500`)
- Con valor → círculo verde (`bg-green-500`)
- Truncar rutas largas con `...` al centro (no al final) — las rutas muestran la carpeta destino que importa, no el inicio

### ModeSelector

Dos radio buttons con descripciones inline. Ninguno seleccionado por defecto. Estilos:
- Radio no seleccionado: borde zinc
- Incremental seleccionado: borde verde, fondo verde/10
- Mirror seleccionado: borde amber, fondo amber/10 — color de advertencia, no rojo (el rojo es para errores críticos)

Texto de cada opción:

**🔄 Copia Incremental**
> Copia archivos nuevos o modificados. No elimina nada en destino.
> `flags: /E /W:1 /R:1`

**🪞 Copia Espejo (Mirror)**
> Destino quedará idéntico al origen. Los archivos borrados en origen se eliminarán en destino.
> `flags: /MIR /W:1 /R:1`

### CommandPreview

Componente de solo lectura. Recibe el `FormState` y genera el display coloreado.

Tokens y colores (paleta VS Code Dark+):
- `robocopy` → `#569CD6` (azul — keyword)
- Ruta Origen → `#4EC994` (verde)
- Ruta Destino → `#CE9178` (naranja/salmón)
- Flags (`/E`, `/MIR`, `/W:1`, `/R:1`) → `#9CDCFE` (celeste — parámetros)

Debajo del comando, una línea en `text-zinc-400` con el resumen en lenguaje humano:
- Incremental: "Copiará archivos nuevos y modificados. No se eliminará nada en destino."
- Mirror: "Destino quedará idéntico a origen. Archivos ausentes en origen serán eliminados."
- Sin modo seleccionado: "Selecciona un modo para ver el comando."

Implementación: no requiere librería externa. El comando se construye como un array de tokens con su color asignado, renderizado como `<span>` con `style={{ color }}`.

### Stubs en `lib/robocopy.ts`

```typescript
export async function runRobocopyPreview(_params: RobocopyParams): Promise<ScanResult> {
  // Stub — reemplazado en spec003
  return { fileCount: 0, totalBytes: 0 }
}

export async function runRobocopy(_params: RobocopyParams): Promise<void> {
  // Stub — reemplazado en spec003
}
```

Esto permite que el botón GO! exista y dispare algo sin que el spec002 dependa del spec003.

### Validación y GO!

El botón GO! tiene tres estados:
- `disabled` con opacity reducida → cuando `isValid === false`
- `enabled` → cuando todo está completo
- `loading` → cuando el proceso fue iniciado (spin icon + "Preparando...")

Al presionar GO! con todo válido: llamar `runRobocopyPreview` (stub) → en spec004 esto transiciona a la pantalla de animación.

---

## Tasks

- [ ] Crear hook `useMainForm.ts` con estado del formulario y lógica `isValid`
- [ ] Crear `PathPicker.tsx` con integración real de `@tauri-apps/plugin-dialog`
- [ ] Implementar indicadores visuales verde/rojo en PathPicker
- [ ] Crear `ModeSelector.tsx` con radio buttons Incremental / Mirror y sus descripciones
- [ ] Verificar que ninguno está seleccionado por defecto y que `isValid` respeta esto
- [ ] Crear `ExcludePathsList.tsx` con input + botón agregar + lista removible
- [ ] Crear `CommandPreview.tsx` con tokens coloreados (sin librería externa)
- [ ] Verificar que CommandPreview refleja cambios de estado en tiempo real
- [ ] Crear `GoButton.tsx` con los tres estados: disabled / enabled / loading
- [ ] Crear stubs en `src/lib/robocopy.ts`
- [ ] Ensamblar todo en `MainScreen.tsx`
- [ ] Revisión visual final: coherencia de colores, legibilidad, espaciado
- [ ] Commit: `feat: main screen UI with command preview and form validation`

---

*Spec 002 — Plaggiarism*
