import { save, open } from "@tauri-apps/plugin-dialog";
import { CopyMode, RobocopyParams, readTextFile, writeTextFile } from "./robocopy";

export interface PlaggConfig {
  origen: string;
  destino: string;
  modo: CopyMode;
  excluir: string[];
}

export function toPlaggConfig(params: RobocopyParams): PlaggConfig {
  return {
    origen: params.origen,
    destino: params.destino,
    modo: params.modo,
    excluir: [...params.excluir],
  };
}

export function parsePlaggConfig(raw: string): PlaggConfig {
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch {
    throw new Error("El archivo no es JSON válido.");
  }
  if (typeof data !== "object" || data === null) {
    throw new Error("Formato .plagg inválido.");
  }
  const d = data as Record<string, unknown>;
  if (typeof d.origen !== "string" || !d.origen.trim()) {
    throw new Error("Falta el campo 'origen'.");
  }
  if (typeof d.destino !== "string" || !d.destino.trim()) {
    throw new Error("Falta el campo 'destino'.");
  }
  if (d.modo !== "incremental" && d.modo !== "mirror") {
    throw new Error("El campo 'modo' debe ser 'incremental' o 'mirror'.");
  }
  if (
    !Array.isArray(d.excluir) ||
    !d.excluir.every((x) => typeof x === "string")
  ) {
    throw new Error("El campo 'excluir' debe ser un arreglo de rutas.");
  }
  return {
    origen: d.origen,
    destino: d.destino,
    modo: d.modo,
    excluir: d.excluir as string[],
  };
}

function sugerirNombre(origen: string): string {
  const base = origen.split(/[\\/]/).filter(Boolean).pop() ?? "respaldo";
  const fecha = new Date().toISOString().slice(0, 10);
  return `respaldo-${base}-${fecha}.plagg`;
}

export async function exportPlagg(
  params: RobocopyParams
): Promise<string | null> {
  const path = await save({
    defaultPath: sugerirNombre(params.origen),
    filters: [{ name: "Plaggiarism Config", extensions: ["plagg"] }],
  });
  if (!path) return null;
  const config = toPlaggConfig(params);
  await writeTextFile(path, JSON.stringify(config, null, 2));
  return path;
}

export async function importPlagg(): Promise<PlaggConfig | null> {
  const path = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Plaggiarism Config", extensions: ["plagg"] }],
  });
  if (!path) return null;
  const raw = await readTextFile(path as string);
  return parsePlaggConfig(raw);
}
