import { useState } from "react";
import { PathPicker } from "../components/PathPicker";
import { ModeSelector } from "../components/ModeSelector";
import { ExcludePathsList } from "../components/ExcludePathsList";
import { CommandPreview } from "../components/CommandPreview";
import { GoButton } from "../components/GoButton";
import { UseMainForm } from "../hooks/useMainForm";
import { RobocopyParams, validatePaths } from "../lib/robocopy";
import { exportPlagg, importPlagg } from "../lib/plagg";

interface MainScreenProps {
  form: UseMainForm;
  onGo: (params: RobocopyParams) => void;
}

export function MainScreen({ form, onGo }: MainScreenProps) {
  const [configMsg, setConfigMsg] = useState<string | null>(null);
  const [goError, setGoError] = useState<string | null>(null);

  async function handleGo() {
    if (!form.isValid) return;
    const params = form.toRobocopyParams();
    const { origenOk, destinoOk } = await validatePaths(
      params.origen,
      params.destino
    );
    if (!origenOk || !destinoOk) {
      const fallas = [
        !origenOk ? "el origen no existe o no es accesible" : null,
        !destinoOk ? "el destino no existe o no es accesible" : null,
      ]
        .filter(Boolean)
        .join(" y ");
      setGoError(`No se puede iniciar la copia: ${fallas}.`);
      return;
    }
    setGoError(null);
    onGo(params);
  }

  async function handleExport() {
    if (!form.isValid) return;
    try {
      const path = await exportPlagg(form.toRobocopyParams());
      setConfigMsg(path ? `Guardado en ${path}` : null);
    } catch (e) {
      setConfigMsg(`No se pudo exportar: ${String(e)}`);
    }
  }

  async function handleImport() {
    try {
      const config = await importPlagg();
      if (!config) return;
      form.setOrigen(config.origen);
      form.setDestino(config.destino);
      form.setModo(config.modo);
      form.setExcluir(config.excluir);
      setConfigMsg("Configuración importada.");
    } catch (e) {
      setConfigMsg(`No se pudo importar: ${String(e)}`);
    }
  }

  return (
    <div className="h-screen overflow-y-auto bg-zinc-900 text-white">
      <div className="mx-auto flex min-h-full max-w-3xl flex-col gap-6 px-6 py-8">
        <header className="flex items-baseline justify-between border-b border-zinc-800 pb-3">
          <h1 className="text-xl font-bold tracking-tight">
            <span className="text-indigo-400">P</span>
            <span>lagg</span>
            <span className="text-green-300">.i</span>
            <span className="text-cyan-300">o</span>
          </h1>
          <span className="text-xs text-zinc-500">robocopy con super poderes</span>
        </header>

        <section className="flex flex-col gap-4">
          <PathPicker
            label="Origen"
            value={form.state.origen}
            onChange={form.setOrigen}
          />
          <PathPicker
            label="Destino"
            value={form.state.destino}
            onChange={form.setDestino}
          />
        </section>

        <ModeSelector value={form.state.modo} onChange={form.setModo} />

        <ExcludePathsList
          paths={form.state.excluir}
          onAdd={form.addExclusion}
          onRemove={form.removeExclusion}
        />

        <CommandPreview
          origen={form.state.origen}
          destino={form.state.destino}
          modo={form.state.modo}
          excluir={form.state.excluir}
        />

        <div className="flex items-center justify-between gap-3">
          <div className="flex gap-2">
            <button
              type="button"
              onClick={handleImport}
              className="rounded-md border border-zinc-700 px-3 py-2 text-xs font-bold text-zinc-300 hover:bg-zinc-800"
            >
              Importar .plagg
            </button>
            <button
              type="button"
              onClick={handleExport}
              disabled={!form.isValid}
              className="rounded-md border border-zinc-700 px-3 py-2 text-xs font-bold text-zinc-300 hover:bg-zinc-800 disabled:opacity-50"
            >
              Exportar .plagg
            </button>
          </div>
          <GoButton disabled={!form.isValid} onGo={handleGo} />
        </div>

        {goError && (
          <p className="rounded-md border border-red-800 bg-red-900/30 px-3 py-2 font-mono text-xs text-red-300">
            {goError}
          </p>
        )}

        {configMsg && (
          <p className="font-mono text-xs text-zinc-400 break-all">{configMsg}</p>
        )}

        <footer className="mt-auto pt-4 text-center text-[11px] text-zinc-600">
          Plaggiarism — v0.1 en construcción
        </footer>
      </div>
    </div>
  );
}
