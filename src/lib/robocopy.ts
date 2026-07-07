import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
  slow: boolean;
}

export interface PathsValidation {
  origenOk: boolean;
  destinoOk: boolean;
}

export type RobocopyStatus =
  | "NothingToDo"
  | "Success"
  | "ExtraFiles"
  | "SuccessWithExtra"
  | "SomeFailed"
  | "FatalError"
  | { Unknown: number };

export interface RobocopyResult {
  status: RobocopyStatus;
  copied: number;
  skipped: number;
  failed: number;
  failedFiles: string[];
  durationSecs: number;
}

export interface FileCompletedEvent {
  remaining: number;
  fileName: string;
}

export interface CopyErrorEvent {
  fileName: string;
  errorCode: number;
}

export const FLAGSETS: Record<CopyMode, string[]> = {
  incremental: ["/E", "/W:1", "/R:1"],
  mirror: ["/MIR", "/W:1", "/R:1"],
};

export const MODE_DESCRIPTIONS: Record<CopyMode, string> = {
  incremental:
    "Copiar\u00e1 archivos nuevos y modificados. No se eliminar\u00e1 nada en destino.",
  mirror:
    "Destino quedar\u00e1 id\u00e9ntico a origen. Archivos ausentes en origen ser\u00e1n eliminados.",
};

export async function validatePaths(
  origen: string,
  destino: string
): Promise<PathsValidation> {
  return invoke<PathsValidation>("validate_paths", { origen, destino });
}

export async function scanRobocopy(params: RobocopyParams): Promise<ScanResult> {
  return invoke<ScanResult>("scan_robocopy", { params });
}

export async function pollScan(): Promise<ScanResult | null> {
  return invoke<ScanResult | null>("poll_scan");
}

export async function runRobocopy(
  params: RobocopyParams,
  totalFiles: number
): Promise<RobocopyResult> {
  return invoke<RobocopyResult>("run_robocopy", {
    params,
    totalFiles,
  });
}

export async function cancelRobocopy(): Promise<void> {
  await invoke<void>("cancel_robocopy");
}

export async function onFileCompleted(
  cb: (data: FileCompletedEvent) => void
): Promise<UnlistenFn> {
  return listen<FileCompletedEvent>("file_completed", (e) => cb(e.payload));
}

export async function onCopyError(
  cb: (data: CopyErrorEvent) => void
): Promise<UnlistenFn> {
  return listen<CopyErrorEvent>("copy_error", (e) => cb(e.payload));
}

export async function onCopyDone(
  cb: (data: RobocopyResult) => void
): Promise<UnlistenFn> {
  return listen<RobocopyResult>("copy_done", (e) => cb(e.payload));
}