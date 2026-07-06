# Spec 001 — Tauri Scaffold

## Propósito

Inicializar el proyecto Plaggiarism como una aplicación Tauri v2 funcional, con el stack frontend (React + TypeScript + TailwindCSS) configurado y corriendo. Al cerrar este spec, el developer debe poder ejecutar `pnpm tauri dev` y ver una ventana de aplicación sin errores, con la estructura de carpetas limpia y lista para recibir features.

---

## Scope

- Inicialización del proyecto con Tauri v2 + React + TypeScript
- Configuración de TailwindCSS v3
- Configuración de ventana (título, tamaño, sin menú nativo)
- Eliminación de boilerplate por defecto de Tauri
- Placeholder de pantalla principal (componente vacío `MainScreen`)
- Primer commit significativo al repositorio

## Fuera de scope

- Ningún componente funcional de UI
- Ningún Tauri command en Rust
- Ninguna lógica de negocio

---

## Plan técnico

### Inicialización

Usar el scaffolding oficial de Tauri v2:

```bash
pnpm create tauri-app@latest
# Seleccionar: React + TypeScript
```

Esto genera la estructura base:
```
plaggiarism/
├── src/               # React + TypeScript
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/         # Rust
│   ├── src/main.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── index.html
├── package.json
└── vite.config.ts
```

### TailwindCSS

Instalar y configurar Tailwind v3 sobre Vite. Verificar que las clases de utilidad se aplican correctamente antes de avanzar — esto evita debugging posterior al tener features encima.

```bash
pnpm add -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

Configurar `content` en `tailwind.config.js` para cubrir `src/**/*.{ts,tsx}`.

### Configuración de ventana

En `tauri.conf.json`, ajustar:
- `title`: "Plaggiarism"
- `width`: 960, `height`: 720
- `resizable`: true
- `decorations`: true (mantener barra de título nativa por ahora)
- Desactivar menú nativo del sistema (`menu: null`)

### Limpieza de boilerplate

Eliminar:
- El componente `Greet` y su lógica asociada en Rust
- El CSS por defecto de Tauri
- Cualquier asset de ejemplo (logo de Tauri, Vite, React)

Dejar en `App.tsx` solo el render de `<MainScreen />`.

### Placeholder MainScreen

Componente funcional mínimo en `src/screens/MainScreen.tsx`:

```tsx
export function MainScreen() {
  return (
    <div className="h-screen bg-zinc-900 text-white flex items-center justify-center">
      <p className="text-zinc-400">Plaggiarism — v0.1 en construcción</p>
    </div>
  )
}
```

Fondo oscuro desde el primer render — coherente con la estética VS Code que tendrá el Command Preview.

### Git

Commit inicial con mensaje: `feat: tauri v2 scaffold with react + typescript + tailwind`.

---

## Tasks

- [ ] Ejecutar `pnpm create tauri-app@latest` con template React + TypeScript
- [ ] Instalar y configurar TailwindCSS v3
- [ ] Verificar que una clase Tailwind de prueba (`bg-zinc-900`) se renderiza correctamente
- [ ] Editar `tauri.conf.json`: título, tamaño de ventana, sin menú nativo
- [ ] Eliminar todo el boilerplate de Tauri (Greet, assets por defecto, CSS de ejemplo)
- [ ] Crear `src/screens/MainScreen.tsx` con placeholder mínimo
- [ ] Actualizar `App.tsx` para renderizar solo `<MainScreen />`
- [ ] Verificar que `pnpm tauri dev` corre sin errores ni warnings relevantes
- [ ] Commit: `feat: tauri v2 scaffold with react + typescript + tailwind`

---

*Spec 001 — Plaggiarism*
