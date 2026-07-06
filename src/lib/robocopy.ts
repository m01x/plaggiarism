export type CopyMode = "incremental" | "mirror";

export interface RobocopyParams {
  origen: string;
  destino: string;
  modo: CopyMode;
  excluir: string[];
}

export interface ScanResult {
  fileCount: number;
  totalBytes: number;
}

export const FLAGSETS: Record<CopyMode, string[]> = {
  incremental: ["/E", "/W:1", "/R:1"],
  mirror: ["/MIR", "/W:1", "/R:1"],
};

export const MODE_DESCRIPTIONS: Record<CopyMode, string> = {
  incremental:
    "Copiará archivos nuevos y modificados. No se eliminará nada en destino.",
  mirror:
    "Destino quedará idéntico a origen. Archivos ausentes en origen serán eliminados.",
};

export async function runRobocopyPreview(
  _params: RobocopyParams
): Promise<ScanResult> {
  return { fileCount: 0, totalBytes: 0 };
}

export async function runRobocopy(_params: RobocopyParams): Promise<void> {}