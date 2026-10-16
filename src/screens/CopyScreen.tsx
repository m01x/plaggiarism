import { useEffect, useRef, useState } from "react";
import type { CopyState } from "../hooks/useCopyProcess";

interface CopyScreenProps {
  state: CopyState;
  onCancel: () => void;
  onConfirmMirror: () => void;
  onAbortMirror: () => void;
}

const PALETTE_NORMAL = ["#569CD6", "#9CDCFE", "#ffffff"];
const RED = "#F44747";

const GENERIC_WORDS = [
  "0x",
  "\\\\",
  ".bak",
  ".tmp",
  ">>",
  "C:\\",
  "idx",
  "2026",
  ".log",
  "/E",
  "100%",
  ".dat",
];

interface Particle {
  angle: number;
  radius: number; // 1 = borde, 0 = centro
  speed: number;
  text: string;
  color: string;
  px: number;
  py: number;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes;
  let u = -1;
  do {
    v /= 1024;
    u += 1;
  } while (v >= 1024 && u < units.length - 1);
  return `${v.toFixed(1)} ${units[u]}`;
}

export function CopyScreen({
  state,
  onCancel,
  onConfirmMirror,
  onAbortMirror,
}: CopyScreenProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const modeRef = useRef<"normal" | "fatal">("normal");
  const redUntilRef = useRef(0);
  const filenameQueueRef = useRef<string[]>([]);
  const [flash, setFlash] = useState(false);

  const isCopying = state.status === "copying";
  const isFatal = state.status === "fatal";
  const lastFile = isCopying ? state.lastFile : "";
  const errorCount = isCopying ? state.errorCount : 0;
  const currentFile = isCopying ? state.currentFile : "";
  const currentPercent = isCopying ? state.currentPercent : 0;

  // Actualizar el modo según el estado.
  useEffect(() => {
    modeRef.current = isFatal ? "fatal" : "normal";
  }, [isFatal]);

  // Alimentar nombres de archivo reales.
  useEffect(() => {
    if (lastFile) filenameQueueRef.current.push(lastFile);
  }, [lastFile]);

  // Flash rojo + tinte en errores parciales.
  useEffect(() => {
    if (isCopying && errorCount > 0) {
      redUntilRef.current = performance.now() + 1500;
      setFlash(true);
      const t = setTimeout(() => setFlash(false), 200);
      return () => clearTimeout(t);
    }
  }, [isCopying, errorCount]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let width = 0;
    let height = 0;
    const dpr = Math.max(1, window.devicePixelRatio || 1);

    function resize() {
      width = window.innerWidth;
      height = window.innerHeight;
      canvas!.width = width * dpr;
      canvas!.height = height * dpr;
      canvas!.style.width = `${width}px`;
      canvas!.style.height = `${height}px`;
      ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    resize();
    window.addEventListener("resize", resize);

    const N = 200;
    const particles: Particle[] = [];
    for (let i = 0; i < N; i++) {
      particles.push(spawnParticle(false));
    }

    let raf = 0;
    let t0 = performance.now();

    function spawnParticle(red: boolean): Particle {
      const angle = Math.random() * Math.PI * 2;
      const text = nextWord(red);
      const color = red
        ? RED
        : PALETTE_NORMAL[Math.floor(Math.random() * PALETTE_NORMAL.length)];
      return {
        angle,
        radius: 0.9 + Math.random() * 0.1,
        speed: 0.004 + Math.random() * 0.006,
        text,
        color,
        px: 0,
        py: 0,
      };
    }

    function nextWord(red: boolean): string {
      const useFile = !red && Math.random() < 0.4 && filenameQueueRef.current.length > 0;
      if (useFile) {
        const f = filenameQueueRef.current.shift();
        if (f) return f;
      }
      return GENERIC_WORDS[Math.floor(Math.random() * GENERIC_WORDS.length)];
    }

    function reset(p: Particle) {
      const red = modeRef.current === "fatal" || performance.now() < redUntilRef.current;
      const np = spawnParticle(red);
      p.angle = np.angle;
      p.radius = np.radius;
      p.speed = np.speed;
      p.text = np.text;
      p.color = np.color;
    }

    function frame(now: number) {
      const dt = Math.min(2, (now - t0) / 16.666);
      t0 = now;
      const cx = width / 2 + Math.sin(now / 600) * 5;
      const cy = height / 2 + Math.cos(now / 800) * 5;
      const R = Math.max(width, height) * 0.75;
      const fatal = modeRef.current === "fatal";
      const red = fatal || now < redUntilRef.current;

      ctx!.fillStyle = "rgba(0,0,0,0.15)";
      ctx!.fillRect(0, 0, width, height);

      for (const p of particles) {
        if (fatal) {
          p.speed *= 0.97;
          p.radius += p.speed * 0.4 * dt;
        } else {
          p.speed *= 1.0015;
          p.radius -= p.speed * dt;
        }

        if (fatal && p.radius > 1.6) {
          reset(p);
        } else if (!fatal && p.radius <= 0) {
          reset(p);
        }

        const x = cx + Math.cos(p.angle) * p.radius * R;
        const y = cy + Math.sin(p.angle) * p.radius * R;

        const near = 1 - Math.min(1, p.radius);
        const alpha = fatal ? Math.max(0, 0.9 - p.radius * 0.5) : 0.15 + near * 0.85;

        ctx!.globalAlpha = alpha;
        ctx!.strokeStyle = red ? RED : p.color;
        ctx!.lineWidth = 1;
        ctx!.beginPath();
        ctx!.moveTo(p.px, p.py);
        ctx!.lineTo(x, y);
        ctx!.stroke();

        ctx!.fillStyle = red ? RED : p.color;
        ctx!.font = `${10 + near * 8}px monospace`;
        ctx!.fillText(p.text, x, y);

        p.px = x;
        p.py = y;
      }

      ctx!.globalAlpha = 1;
      raf = requestAnimationFrame(frame);
    }

    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
    };
  }, []);

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-black text-white">
      <canvas ref={canvasRef} className="absolute inset-0" />

      {flash && (
        <div className="pointer-events-none absolute inset-0 ring-8 ring-red-500/70" />
      )}

      <div className="pointer-events-none absolute inset-x-0 bottom-10 flex flex-col items-center gap-1 font-mono">
        {state.status === "scanning" && (
          <span className="text-zinc-300">Escaneando archivos…</span>
        )}
        {state.status === "scanning_slow" && (
          <span className="text-zinc-300">
            Calculando archivos… Tu respaldo es grande, por favor ten paciencia.
          </span>
        )}
        {isCopying && (
          <>
            <span className="text-zinc-200">
              Archivos restantes:{" "}
              <span className="font-bold text-cyan-300">
                {state.remaining}
              </span>{" "}
              de {state.total}
            </span>
            {currentFile && (
              <span className="text-zinc-300">
                Copiando{" "}
                <span className="text-cyan-200">{currentFile}</span>…{" "}
                {currentPercent}%
              </span>
            )}
            {state.errorCount > 0 && (
              <span className="text-red-400">
                {state.errorCount} archivo{state.errorCount === 1 ? "" : "s"} con
                error
              </span>
            )}
          </>
        )}
      </div>

      {isCopying && (
        <button
          type="button"
          onClick={onCancel}
          className="absolute bottom-6 right-6 rounded-md bg-red-700 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-red-600"
        >
          Cancelar
        </button>
      )}

      {isFatal && (
        <div className="absolute inset-0 flex items-center justify-center bg-black/40">
          <div className="max-w-md rounded-md border border-red-900 bg-zinc-900/90 p-6 text-center">
            <div className="mb-3 text-4xl">✕</div>
            <h2 className="mb-2 text-lg font-bold text-red-400">
              La copia falló
            </h2>
            <p className="mb-4 break-words font-mono text-sm text-zinc-300">
              {state.message}
            </p>
            <button
              type="button"
              onClick={onAbortMirror}
              className="rounded-md bg-zinc-700 px-4 py-2 text-sm font-bold text-white hover:bg-zinc-600"
            >
              Volver al inicio
            </button>
          </div>
        </div>
      )}

      {state.status === "awaiting_confirmation" && (
        <div className="absolute inset-0 flex items-center justify-center bg-black/60">
          <div className="max-w-md rounded-md border border-amber-700 bg-zinc-900 p-6">
            <h2 className="mb-3 flex items-center gap-2 text-lg font-bold text-amber-400">
              ⚠️ Modo Espejo — Confirmación requerida
            </h2>
            <p className="mb-4 text-sm text-zinc-300">
              Este modo eliminará{" "}
              <span className="font-bold text-amber-300">
                {state.extrasCount}
              </span>{" "}
              archivo{state.extrasCount === 1 ? "" : "s"} en destino que no
              existe{state.extrasCount === 1 ? "" : "n"} en el origen
              seleccionado.
            </p>
            <div className="flex justify-end gap-3">
              <button
                type="button"
                onClick={onAbortMirror}
                className="rounded-md bg-zinc-700 px-4 py-2 text-sm font-bold text-white hover:bg-zinc-600"
              >
                Cancelar
              </button>
              <button
                type="button"
                onClick={onConfirmMirror}
                className="rounded-md bg-amber-600 px-4 py-2 text-sm font-bold text-white hover:bg-amber-500"
              >
                Confirmar y continuar
              </button>
            </div>
          </div>
        </div>
      )}

      {state.status === "insufficient_space" && (
        <div className="absolute inset-0 flex items-center justify-center bg-black/60">
          <div className="max-w-md rounded-md border border-red-800 bg-zinc-900 p-6">
            <h2 className="mb-3 flex items-center gap-2 text-lg font-bold text-red-400">
              ⚠️ Espacio insuficiente en destino
            </h2>
            <p className="mb-4 text-sm text-zinc-300">
              La copia necesita{" "}
              <span className="font-bold text-red-300">
                {formatBytes(state.needed)}
              </span>
              , pero el destino solo tiene{" "}
              <span className="font-bold text-zinc-100">
                {formatBytes(state.available)}
              </span>{" "}
              libres.
            </p>
            <div className="flex justify-end gap-3">
              <button
                type="button"
                onClick={onAbortMirror}
                className="rounded-md bg-zinc-700 px-4 py-2 text-sm font-bold text-white hover:bg-zinc-600"
              >
                Volver al inicio
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
