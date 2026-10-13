import { useState } from "react";
import { useMainForm } from "./hooks/useMainForm";
import { useCopyProcess } from "./hooks/useCopyProcess";
import { RobocopyParams } from "./lib/robocopy";
import { MainScreen } from "./screens/MainScreen";
import { CopyScreen } from "./screens/CopyScreen";
import { ResultScreen } from "./screens/ResultScreen";

function App() {
  const form = useMainForm();
  const copy = useCopyProcess();
  const [params, setParams] = useState<RobocopyParams | null>(null);

  function handleGo(p: RobocopyParams) {
    setParams(p);
    copy.start(p);
  }

  if (copy.state.status === "done" && params) {
    return (
      <ResultScreen
        params={params}
        result={copy.state.result}
        onRestart={copy.reset}
      />
    );
  }

  if (copy.state.status !== "idle") {
    return (
      <CopyScreen
        state={copy.state}
        onCancel={copy.cancel}
        onConfirmMirror={copy.confirmMirror}
        onAbortMirror={copy.abortMirror}
      />
    );
  }

  return <MainScreen form={form} onGo={handleGo} />;
}

export default App;
