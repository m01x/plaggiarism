import { useCallback, useMemo, useState } from "react";
import {
  CopyMode,
  RobocopyParams,
  FLAGSETS,
} from "../lib/robocopy";

export interface FormState {
  origen: string;
  destino: string;
  modo: CopyMode | null;
  excluir: string[];
}

export interface UseMainForm {
  state: FormState;
  isValid: boolean;
  commandString: string;
  setOrigen: (v: string) => void;
  setDestino: (v: string) => void;
  setModo: (m: CopyMode) => void;
  addExclusion: (p: string) => void;
  removeExclusion: (p: string) => void;
  toRobocopyParams: () => RobocopyParams;
}

function buildCommand(s: FormState): string {
  if (!s.origen || !s.destino || !s.modo) return "";
  const flags = FLAGSETS[s.modo].join(" ");
  const exclude =
    s.excluir.length > 0
      ? " " + s.excluir.map((p) => `/XD "${p}"`).join(" ")
      : "";
  return `robocopy "${s.origen}" "${s.destino}" ${flags}${exclude}`.trim();
}

export function useMainForm(): UseMainForm {
  const [state, setState] = useState<FormState>({
    origen: "",
    destino: "",
    modo: null,
    excluir: [],
  });

  const setOrigen = useCallback((v: string) => {
    setState((s) => ({ ...s, origen: v }));
  }, []);
  const setDestino = useCallback((v: string) => {
    setState((s) => ({ ...s, destino: v }));
  }, []);
  const setModo = useCallback((m: CopyMode) => {
    setState((s) => ({ ...s, modo: m }));
  }, []);
  const addExclusion = useCallback((p: string) => {
    const trimmed = p.trim();
    if (!trimmed) return;
    setState((s) =>
      s.excluir.includes(trimmed)
        ? s
        : { ...s, excluir: [...s.excluir, trimmed] }
    );
  }, []);
  const removeExclusion = useCallback((p: string) => {
    setState((s) => ({
      ...s,
      excluir: s.excluir.filter((x) => x !== p),
    }));
  }, []);

  const toRobocopyParams = useCallback(
    (): RobocopyParams => ({
      origen: state.origen,
      destino: state.destino,
      modo: (state.modo ?? "incremental") as CopyMode,
      excluir: state.excluir,
    }),
    [state]
  );

  const isValid = useMemo(() => {
    return Boolean(state.origen && state.destino && state.modo);
  }, [state.origen, state.destino, state.modo]);

  const commandString = useMemo(() => buildCommand(state), [state]);

  return {
    state,
    isValid,
    commandString,
    setOrigen,
    setDestino,
    setModo,
    addExclusion,
    removeExclusion,
    toRobocopyParams,
  };
}