import { Fragment, useState } from "react";
import { buildTokens } from "../components/CommandPreview";
import {
  RobocopyParams,
  RobocopyResult,
  RobocopyStatus,
} from "../lib/robocopy";
import { exportPlagg } from "../lib/plagg";

interface ResultScreenProps {
  params: RobocopyParams;
  result: RobocopyResult;
  onRestart: () => void;
}

interface StatusInfo {
  icon: string;
  color: string;
  title: string;
}

function statusInfo(status: RobocopyStatus): StatusInfo {
  if (typeof status === "object" && "Unknown" in status) {
    return {
      icon: "✕",
      color: "#F44747",
      title: "Copia cancelada o finalizada inesperadamente",
    };
  }
  switch (status) {
    case "Success":
    case "SuccessWithExtra":
      return { icon: "✓", color: "#4EC994", title: "Copia completada" };
    case "NothingToDo":
      return {
        icon: "○",
        color: "#9ca3af",
        title: "Destino ya estaba actualizado",
      };
    case "ExtraFiles":
      return {
        icon: "✓",
        color: "#4EC994",
        title: "Copia completada (archivos extra en destino)",
      };
    case "SomeFailed":
      return { icon: "⚠", color: "#d4a72c", title: "Copia con errores" };
    case "FatalError":
      return { icon: "✕", color: "#F44747", title: "Error crítico" };
  }
}

function formatDuration(secs: number): string {
  const s = Math.round(secs);
  const m = Math.floor(s / 60);
  const r = s % 60;
  if (m === 0) return `${r}s`;
  return `${m}m ${r}s`;
}

export function ResultScreen({
  params,
  result,
  onRestart,
}: ResultScreenProps) {
  const [showFailed, setShowFailed] = useState(false);
  const [exportMsg, setExportMsg] = useState<string | null>(null);
  const info = statusInfo(result.status);

  async function handleExport() {
    try {
      const path = await exportPlagg(params);
      setExportMsg(path ? `Guardado en ${path}` : null);
    } catch (e) {
      setExportMsg(`No se pudo exportar: ${String(e)}`);
    }
  }

  return (
    <div className="h-screen overflow-y-auto bg-zinc-900 text-white">
      <div className="mx-auto flex min-h-full max-w-3xl flex-col gap-6 px-6 py-8">
        <header className="flex items-center gap-3 border-b border-zinc-800 pb-3">
          <span
            className="text-3xl font-bold"
            style={{ color: info.color }}
            aria-hidden
          >
            {info.icon}
          </span>
          <h1 className="text-xl font-bold tracking-tight">{info.title}</h1>
        </header>

        <section className="rounded-md border border-zinc-800 bg-[#1e1e1e] p-3">
          <pre className="overflow-x-auto font-mono text-sm leading-relaxed whitespace-pre-wrap break-words">
            <code>
              {buildTokens(params.origen, params.destino, params.modo, params.excluir).map(
                (t, i) => (
                  <Fragment key={i}>
                    {i > 0 ? " " : ""}
                    <span style={{ color: t.color }} title={t.title}>
                      {t.text}
                    </span>
                  </Fragment>
                )
              )}
            </code>
          </pre>
        </section>

        <section className="flex flex-col gap-2 font-mono text-sm">
          <div className="flex justify-between">
            <span className="text-zinc-400">Copiados:</span>
            <span className="text-green-300">{result.copied}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-zinc-400">Saltados:</span>
            <span className="text-zinc-300">{result.skipped}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-zinc-400">Fallidos:</span>
            <span className={result.failed > 0 ? "text-red-400" : "text-zinc-300"}>
              {result.failed}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-zinc-400">Duración:</span>
            <span className="text-zinc-300">{formatDuration(result.durationSecs)}</span>
          </div>
        </section>

        {result.failed > 0 && (
          <section className="flex flex-col gap-2">
            <button
              type="button"
              onClick={() => setShowFailed((v) => !v)}
              className="self-start text-sm font-medium text-red-400 hover:text-red-300"
            >
              {showFailed ? "▾" : "▸"} Ver archivos con error ({result.failed})
            </button>
            {showFailed && (
              <ul className="rounded-md border border-zinc-800 bg-[#1e1e1e] p-3 font-mono text-xs text-red-400">
                {result.failedFiles.map((f, i) => (
                  <li key={i} className="break-all">
                    {f}
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}

        {exportMsg && (
          <p className="font-mono text-xs text-zinc-400 break-all">{exportMsg}</p>
        )}

        <div className="mt-auto flex justify-end gap-3 pt-4">
          <button
            type="button"
            onClick={handleExport}
            className="rounded-md bg-zinc-700 px-4 py-2 text-sm font-bold text-white hover:bg-zinc-600"
          >
            Exportar .plagg
          </button>
          <button
            type="button"
            onClick={onRestart}
            className="rounded-md bg-green-600 px-4 py-2 text-sm font-bold text-white hover:bg-green-500"
          >
            Volver
          </button>
        </div>
      </div>
    </div>
  );
}
