import { useState } from "react";
import { PathPicker } from "../components/PathPicker";
import { ModeSelector } from "../components/ModeSelector";
import { ExcludePathsList } from "../components/ExcludePathsList";
import { CommandPreview } from "../components/CommandPreview";
import { GoButton } from "../components/GoButton";
import { useMainForm } from "../hooks/useMainForm";
import { scanRobocopy } from "../lib/robocopy";

export function MainScreen() {
  const form = useMainForm();
  const [loading, setLoading] = useState(false);

  async function handleGo() {
    if (!form.isValid || loading) return;
    setLoading(true);
    try {
      const params = form.toRobocopyParams();
      const scan = await scanRobocopy(params);
      console.info("[plagg] scan:", scan);
    } finally {
      setLoading(false);
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

        <div className="flex justify-end pt-2">
          <GoButton
            disabled={!form.isValid}
            loading={loading}
            onGo={handleGo}
          />
        </div>

        <footer className="mt-auto pt-4 text-center text-[11px] text-zinc-600">
          Plaggiarism — v0.1 en construcción
        </footer>
      </div>
    </div>
  );
}