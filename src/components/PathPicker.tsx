import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

interface PathPickerProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}

function truncateMiddle(path: string, maxLen = 48): string {
  if (path.length <= maxLen) return path;
  const keep = maxLen - 3;
  const head = Math.ceil(keep * 0.45);
  const tail = Math.floor(keep * 0.55);
  return `${path.slice(0, head)}...${path.slice(path.length - tail)}`;
}

export function PathPicker({
  label,
  value,
  onChange,
  placeholder = "Sin selección",
}: PathPickerProps) {
  const [busy, setBusy] = useState(false);
  const isSelected = Boolean(value);

  async function handlePick() {
    if (busy) return;
    setBusy(true);
    try {
      const folder = await open({ directory: true, multiple: false });
      if (typeof folder === "string") onChange(folder);
    } finally {
      setBusy(false);
    }
  }

  const dot = isSelected ? "bg-green-500" : "bg-red-500";
  const ring = isSelected
    ? "border-green-600/50 focus-within:border-green-500"
    : "border-zinc-700 focus-within:border-zinc-500";

  return (
    <div className="flex flex-col gap-1.5">
      <label className="text-sm font-medium text-zinc-300">{label}</label>
      <div
        className={`flex items-center gap-3 rounded-md border ${ring} bg-zinc-800/60 px-3 py-2.5 transition-colors`}
      >
        <span className={`h-3 w-3 shrink-0 rounded-full ${dot}`} />
        <span
          className={`flex-1 truncate font-mono text-sm ${
            isSelected ? "text-zinc-100" : "text-zinc-500"
          }`}
          title={value || undefined}
        >
          {isSelected ? truncateMiddle(value) : placeholder}
        </span>
        <button
          type="button"
          onClick={handlePick}
          disabled={busy}
          className="shrink-0 rounded bg-zinc-700 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-600 disabled:opacity-60"
        >
          {busy ? "…" : "Examinar"}
        </button>
      </div>
    </div>
  );
}