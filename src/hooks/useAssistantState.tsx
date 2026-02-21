import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type AssistantMode =
  | "idle"
  | "listening"
  | "thinking"
  | "speaking"
  | "executing";

export interface TranscriptLog {
  id: string;
  message: string;
  mode: AssistantMode;
  timestamp: string;
}

interface AssistantContextValue {
  state: AssistantMode;
  amplitude: number;
  logs: TranscriptLog[];
  isConsoleOpen: boolean;
  setState: (next: AssistantMode) => void;
  toggleConsole: () => void;
  startListening: () => void;
  stopListening: () => void;
  speak: (text?: string) => void;
  executeTask: (task?: string) => void;
  addLog: (message: string, mode?: AssistantMode) => void;
}

const AssistantStateContext = createContext<AssistantContextValue | null>(null);

function nowTime() {
  return new Date().toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function makeLog(message: string, mode: AssistantMode): TranscriptLog {
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
  return { id, message, mode, timestamp: nowTime() };
}

function amplitudeTargetByState(state: AssistantMode) {
  switch (state) {
    case "idle":
      return 0.08 + Math.random() * 0.06;
    case "listening":
      return 0.35 + Math.random() * 0.45;
    case "thinking":
      return 0.18 + Math.random() * 0.14;
    case "speaking":
      return 0.48 + Math.random() * 0.42;
    case "executing":
      return 0.28 + Math.random() * 0.24;
    default:
      return 0.1;
  }
}

export function AssistantStateProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AssistantMode>("idle");
  const [amplitude, setAmplitude] = useState(0.1);
  const [isConsoleOpen, setIsConsoleOpen] = useState(false);
  const [logs, setLogs] = useState<TranscriptLog[]>([
    makeLog("Sarah AI initialized locally.", "idle"),
    makeLog("Press Space to start listening.", "idle"),
  ]);

  const appendLog = useCallback((message: string, mode: AssistantMode) => {
    setLogs((current) => [...current, makeLog(message, mode)].slice(-180));
  }, []);

  const addLog = useCallback(
    (message: string, mode: AssistantMode = state) => {
      appendLog(message, mode);
    },
    [appendLog, state],
  );

  const startListening = useCallback(() => {
    setState("listening");
    appendLog("Microphone activated. Awaiting command.", "listening");
  }, [appendLog]);

  const stopListening = useCallback(() => {
    setState("idle");
    appendLog("Microphone muted.", "idle");
  }, [appendLog]);

  const speak = useCallback(
    (text = "Speaking response from local model.") => {
      setState("speaking");
      appendLog(text, "speaking");
    },
    [appendLog],
  );

  const executeTask = useCallback(
    (task = "Executing local task pipeline.") => {
      setState("executing");
      appendLog(task, "executing");
    },
    [appendLog],
  );

  const toggleConsole = useCallback(() => {
    setIsConsoleOpen((current) => !current);
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      const target = amplitudeTargetByState(state);
      setAmplitude((current) => current + (target - current) * 0.4);
    }, 60);

    return () => window.clearInterval(timer);
  }, [state]);

  const value = useMemo<AssistantContextValue>(
    () => ({
      state,
      amplitude,
      logs,
      isConsoleOpen,
      setState,
      toggleConsole,
      startListening,
      stopListening,
      speak,
      executeTask,
      addLog,
    }),
    [
      addLog,
      amplitude,
      executeTask,
      isConsoleOpen,
      logs,
      speak,
      startListening,
      state,
      stopListening,
      toggleConsole,
    ],
  );

  return (
    <AssistantStateContext.Provider value={value}>
      {children}
    </AssistantStateContext.Provider>
  );
}

export function useAssistantState() {
  const context = useContext(AssistantStateContext);
  if (!context) {
    throw new Error("useAssistantState must be used inside AssistantStateProvider.");
  }
  return context;
}
