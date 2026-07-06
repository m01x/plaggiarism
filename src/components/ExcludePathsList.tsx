import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

interface ExcludePathsListProps {
  paths: string[];
  onAdd: (p: string) => void;
  onRemove: (p: string) => void;
}

export function ExcludePathsList({
  paths,
  onAdd,
  onRemove,
}: ExcludePathsListProps) {
  const [draft, setDraft] = useState("");
  const [picking, setPicking] = useState(false);

  async function handleBrowse() {
    if (picking) return;
    setPicking(true);
    try {
      const folder = await open({ directory: true, multiple: false });
      if (typeof folder === "string") setDraft(folder);
    } finally {
      setPicking(false);
    }
  }

  function submit() {
    if (!draft.trim()) return;
    onAdd(draft.trim());
    setDraft("");
  }

  return (
    <div className="flex flex-col gap-2">
      <span className="text-sm font-medium text-zinc-300">
        Excluir rutas
      </span>
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
          placeholder="Ruta a excluir (ej. C:\Temp)"
          className="flex-1 rounded-md border border-zinc-700 bg-zinc-800/60 px-3 py-2 font-mono text-sm text-zinc-100 placeholder:text-zinc-600 focus:border-zinc-500 focus:outline-none"
        />
        <button
          type="button"
          onClick={handleBrowse}
          disabled={picking}
          className="shrink-0 rounded bg-zinc-700 px-3 py-2 text-xs font-medium text-zinc-200 hover:bg-zinc-600 disabled:opacity-60"
        >
          {picking ? "…" : "Examinar"}
        </button>
        <button
          type="button"
          onClick={submit}
          className="shrink-0 rounded bg-zinc-700 px-3 py-2 text-xs font-medium text-zinc-200 hover:bg-zinc-600"
        >
          Agregar
        </button>
      </div>
      {paths.length > 0 && (
        <ul className="flex flex-col gap-1">
          {paths.map((p) => (
            <li
              key={p}
              className="flex items-center justify-between gap-2 rounded bg-zinc-800/40 px-2.5 py-1.5"
            >
              <span className="truncate font-mono text-xs text-zinc-300" title={p}>
                {p}
              </span>
              <button
                type="button"
                onClick={() => onRemove(p)}
                className="shrink-0 text-xs text-zinc-500 hover:text-red-400"
                aria-label={`Quitar ${p}`}
              >
                ✕
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}