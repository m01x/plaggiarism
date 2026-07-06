import { Fragment } from "react";
import {
  CopyMode,
  FLAGSETS,
  MODE_DESCRIPTIONS,
} from "../lib/robocopy";

interface CommandPreviewProps {
  origen: string;
  destino: string;
  modo: CopyMode | null;
  excluir: string[];
}

const COLOR = {
  keyword: "#569CD6",
  origin: "#4EC994",
  dest: "#CE9178",
  flag: "#9CDCFE",
  quote: "#d4d4d4",
  pathArg: "#CE9178",
};

function truncateMiddle(path: string, maxLen = 40): string {
  if (path.length <= maxLen) return path;
  const keep = maxLen - 3;
  const head = Math.ceil(keep * 0.4);
  const tail = Math.floor(keep * 0.6);
  return `${path.slice(0, head)}...${path.slice(path.length - tail)}`;
}

interface Token {
  text: string;
  color: string;
  title?: string;
}

function buildTokens(
  origen: string,
  destino: string,
  modo: CopyMode,
  excluir: string[]
): Token[] {
  const tokens: Token[] = [
    { text: "robocopy", color: COLOR.keyword },
    { text: '"', color: COLOR.quote },
    { text: truncateMiddle(origen), color: COLOR.origin, title: origen },
    { text: '"', color: COLOR.quote },
    { text: '"', color: COLOR.quote },
    { text: truncateMiddle(destino), color: COLOR.dest, title: destino },
    { text: '"', color: COLOR.quote },
  ];
  for (const f of FLAGSETS[modo]) {
    tokens.push({ text: f, color: COLOR.flag, title: flagTip(f) });
  }
  for (const p of excluir) {
    tokens.push({ text: "/XD", color: COLOR.flag, title: flagTip("/XD") });
    tokens.push({ text: '"', color: COLOR.quote });
    tokens.push({ text: truncateMiddle(p), color: COLOR.pathArg, title: p });
    tokens.push({ text: '"', color: COLOR.quote });
  }
  return tokens;
}

export function CommandPreview({
  origen,
  destino,
  modo,
  excluir,
}: CommandPreviewProps) {
  const hasCommand = Boolean(origen && destino && modo);
  const summary = modo
    ? MODE_DESCRIPTIONS[modo]
    : "Selecciona un modo para ver el comando.";

  return (
    <div className="flex flex-col gap-2">
      <span className="text-sm font-medium text-zinc-300">
        Command Preview
      </span>
      <div className="rounded-md border border-zinc-800 bg-[#1e1e1e] p-3 shadow-inner">
        <pre className="overflow-x-auto font-mono text-sm leading-relaxed whitespace-pre-wrap break-words">
          {hasCommand && modo ? (
            <code>
              {buildTokens(origen, destino, modo, excluir).map((t, i) => (
                <Fragment key={i}>
                  {i > 0 ? " " : ""}
                  <span
                    style={{ color: t.color }}
                    title={t.title}
                  >
                    {t.text}
                  </span>
                </Fragment>
              ))}
            </code>
          ) : (
            <code className="text-zinc-600">{"\u00A0"}</code>
          )}
        </pre>
      </div>
      <p className="text-xs leading-snug text-zinc-400">{summary}</p>
    </div>
  );
}

function flagTip(f: string): string {
  switch (f) {
    case "/E":
      return "Copia directorios y subdirectorios, incluyendo los vacíos.";
    case "/MIR":
      return "Espejo: sincroniza destino con origen (elimina lo que falta en origen).";
    case "/W:1":
      return "Espera 1 segundo entre reintentos.";
    case "/R:1":
      return "Reintenta 1 vez en caso de error.";
    case "/XD":
      return "Excluye directorios listados.";
    default:
      return "";
  }
}