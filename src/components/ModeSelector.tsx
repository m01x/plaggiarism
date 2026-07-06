import { CopyMode, FLAGSETS } from "../lib/robocopy";

interface ModeSelectorProps {
  value: CopyMode | null;
  onChange: (m: CopyMode) => void;
}

interface OptionDef {
  key: CopyMode;
  emoji: string;
  title: string;
  desc: string;
  activeClass: string;
}

const OPTIONS: OptionDef[] = [
  {
    key: "incremental",
    emoji: "🔄",
    title: "Copia Incremental",
    desc: "Copia archivos nuevos o modificados. No elimina nada en destino.",
    activeClass: "border-green-500 bg-green-500/10",
  },
  {
    key: "mirror",
    emoji: "🪞",
    title: "Copia Espejo (Mirror)",
    desc: "Destino quedará idéntico al origen. Los archivos borrados en origen se eliminarán en destino.",
    activeClass: "border-amber-500 bg-amber-500/10",
  },
];

export function ModeSelector({ value, onChange }: ModeSelectorProps) {
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-1 text-sm font-medium text-zinc-300">
        Modo de copia
      </legend>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {OPTIONS.map((opt) => {
          const active = value === opt.key;
          const baseClass = !active
            ? "border-zinc-700 bg-zinc-800/40 hover:border-zinc-600"
            : opt.activeClass;
          return (
            <label
              key={opt.key}
              className={`cursor-pointer rounded-md border p-3 transition-colors ${baseClass}`}
            >
              <div className="flex items-start gap-2.5">
                <input
                  type="radio"
                  name="modo"
                  checked={active}
                  onChange={() => onChange(opt.key)}
                  className="mt-0.5 h-4 w-4 shrink-0 accent-green-500"
                />
                <div className="min-w-0">
                  <div className="flex items-center gap-1.5 text-sm font-semibold text-zinc-100">
                    <span aria-hidden>{opt.emoji}</span>
                    <span>{opt.title}</span>
                  </div>
                  <p className="mt-1 text-xs leading-snug text-zinc-400">
                    {opt.desc}
                  </p>
                  <p className="mt-1 font-mono text-[11px] text-cyan-300/80">
                    flags: {FLAGSETS[opt.key].join(" ")}
                  </p>
                </div>
              </div>
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}