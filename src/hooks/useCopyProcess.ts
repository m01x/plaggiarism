import { useCallback, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  RobocopyParams,
  RobocopyResult,
  cancelRobocopy,
  diskFreeSpace,
  onCopyDone,
  onCopyError,
  onFileCompleted,
  onFileProgress,
  pollScan,
  runRobocopy,
  scanRobocopy,
} from "../lib/robocopy";

export type CopyState =
  | { status: "idle" }
  | { status: "scanning" }
  | { status: "scanning_slow" }
  | { status: "awaiting_confirmation"; extrasCount: number }
  | { status: "insufficient_space"; needed: number; available: number }
  | {
      status: "copying";
      remaining: number;
      total: number;
      lastFile: string;
      errorCount: number;
      currentFile: string;
      currentPercent: number;
    }
  | { status: "fatal"; message: string }
  | { status: "done"; result: RobocopyResult };

export interface UseCopyProcess {
  state: CopyState;
  start: (params: RobocopyParams) => void;
  confirmMirror: () => void;
  abortMirror: () => void;
  cancel: () => void;
  reset: () => void;
}

export function useCopyProcess(): UseCopyProcess {
  const [state, setState] = useState<CopyState>({ status: "idle" });
  const unlisteners = useRef<UnlistenFn[]>([]);
  const pending = useRef<{ params: RobocopyParams; total: number } | null>(null);
  const cancelled = useRef(false);

  const cleanup = useCallback(() => {
    for (const un of unlisteners.current) un();
    unlisteners.current = [];
  }, []);

  const runCopy = useCallback(
    async (params: RobocopyParams, total: number) => {
      setState({
        status: "copying",
        remaining: total,
        total,
        lastFile: "",
        errorCount: 0,
        currentFile: "",
        currentPercent: 0,
      });

      const un1 = await onFileCompleted((e) => {
        setState((s) =>
          s.status === "copying"
            ? { ...s, remaining: e.remaining, lastFile: e.fileName }
            : s
        );
      });
      const un2 = await onCopyError(() => {
        setState((s) =>
          s.status === "copying" ? { ...s, errorCount: s.errorCount + 1 } : s
        );
      });
      const un3 = await onFileProgress((e) => {
        setState((s) =>
          s.status === "copying"
            ? { ...s, currentFile: e.fileName, currentPercent: e.percent }
            : s
        );
      });
      const un4 = await onCopyDone((result) => {
        setState({ status: "done", result });
      });
      unlisteners.current = [un1, un2, un3, un4];

      try {
        const result = await runRobocopy(params, total);
        if (cancelled.current) return;
        setState({ status: "done", result });
      } catch (e) {
        if (cancelled.current) return;
        setState({ status: "fatal", message: String(e) });
      } finally {
        cleanup();
      }
    },
    [cleanup]
  );

  const start = useCallback(
    async (params: RobocopyParams) => {
      cancelled.current = false;
      pending.current = null;
      cleanup();
      setState({ status: "scanning" });
      try {
        let scan = await scanRobocopy(params);
        if (scan.slow) {
          setState({ status: "scanning_slow" });
          for (;;) {
            await new Promise((r) => setTimeout(r, 400));
            const p = await pollScan();
            if (p && !p.slow) {
              scan = p;
              break;
            }
          }
        }
        // Chequeo de espacio: si el destino no alcanza para lo que hay que
        // copiar, bloquear antes de lanzar la copia.
        const free = await diskFreeSpace(params.destino);
        if (scan.totalBytes > free) {
          setState({
            status: "insufficient_space",
            needed: scan.totalBytes,
            available: free,
          });
          return;
        }
        if (params.modo === "mirror" && scan.extrasCount > 0) {
          pending.current = { params, total: scan.fileCount };
          setState({
            status: "awaiting_confirmation",
            extrasCount: scan.extrasCount,
          });
          return;
        }
        await runCopy(params, scan.fileCount);
      } catch (e) {
        if (cancelled.current) return;
        setState({ status: "fatal", message: String(e) });
      }
    },
    [cleanup, runCopy]
  );

  const confirmMirror = useCallback(() => {
    const p = pending.current;
    if (!p) return;
    pending.current = null;
    void runCopy(p.params, p.total);
  }, [runCopy]);

  const abortMirror = useCallback(() => {
    pending.current = null;
    setState({ status: "idle" });
  }, []);

  const cancel = useCallback(async () => {
    cancelled.current = true;
    cleanup();
    await cancelRobocopy();
    setState({ status: "idle" });
  }, [cleanup]);

  const reset = useCallback(() => {
    cancelled.current = true;
    cleanup();
    pending.current = null;
    setState({ status: "idle" });
  }, [cleanup]);

  return { state, start, confirmMirror, abortMirror, cancel, reset };
}
