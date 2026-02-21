import { AnimatePresence, motion } from "framer-motion";
import { ChevronUp, TerminalSquare } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useAssistantState } from "@/hooks/useAssistantState";

function ConsoleDrawer() {
  const {
    logs,
    state,
    isConsoleOpen,
    toggleConsole,
    setState,
    addLog,
    startListening,
    stopListening,
    speak,
    executeTask,
  } = useAssistantState();

  return (
    <section className="pointer-events-none fixed inset-x-0 bottom-0 z-20 flex flex-col items-center">
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={toggleConsole}
        className="pointer-events-auto mb-3 rounded-full border-slate-500/40 bg-slate-900/55 text-slate-200 backdrop-blur-md hover:bg-slate-800/70"
      >
        <TerminalSquare className="size-4" />
        Console
        <ChevronUp
          className={`size-4 transition-transform duration-200 ${
            isConsoleOpen ? "rotate-180" : "rotate-0"
          }`}
        />
      </Button>

      <AnimatePresence>
        {isConsoleOpen && (
          <motion.aside
            initial={{ y: "104%", opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: "104%", opacity: 0 }}
            transition={{ duration: 0.3, ease: [0.22, 0.8, 0.28, 1] }}
            className="pointer-events-auto mb-4 h-[min(40vh,360px)] w-[min(95vw,68rem)] overflow-hidden rounded-2xl border border-slate-300/10 bg-slate-950/62 shadow-2xl backdrop-blur-xl"
          >
            <header className="flex flex-wrap items-center justify-between gap-2 border-b border-slate-300/10 px-4 py-3">
              <div>
                <p className="text-sm font-medium text-slate-200">Sarah AI Console</p>
                <p className="text-xs text-slate-400">Transcript and state controls</p>
              </div>

              <div className="flex flex-wrap items-center gap-1">
                <Button
                  type="button"
                  variant={state === "idle" ? "secondary" : "ghost"}
                  size="sm"
                  onClick={() => {
                    setState("idle");
                    addLog("State forced to idle.", "idle");
                  }}
                >
                  Idle
                </Button>
                <Button
                  type="button"
                  variant={state === "listening" ? "secondary" : "ghost"}
                  size="sm"
                  onClick={startListening}
                >
                  Listen
                </Button>
                <Button
                  type="button"
                  variant={state === "thinking" ? "secondary" : "ghost"}
                  size="sm"
                  onClick={() => {
                    setState("thinking");
                    addLog("Analyzing intent...", "thinking");
                  }}
                >
                  Think
                </Button>
                <Button
                  type="button"
                  variant={state === "speaking" ? "secondary" : "ghost"}
                  size="sm"
                  onClick={() => speak()}
                >
                  Speak
                </Button>
                <Button
                  type="button"
                  variant={state === "executing" ? "secondary" : "ghost"}
                  size="sm"
                  onClick={() => executeTask()}
                >
                  Execute
                </Button>
                <Button type="button" variant="ghost" size="sm" onClick={stopListening}>
                  Stop
                </Button>
              </div>
            </header>

            <div className="sarah-console-scroll h-[calc(100%-57px)] overflow-y-auto px-3 py-3">
              <ul className="space-y-2">
                {logs.map((log) => (
                  <li
                    key={log.id}
                    className="rounded-xl border border-slate-200/8 bg-slate-900/45 px-3 py-2"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-[11px] font-medium tracking-[0.08em] text-cyan-300 uppercase">
                        {log.mode}
                      </span>
                      <span className="text-[11px] text-slate-500">{log.timestamp}</span>
                    </div>
                    <p className="mt-1 text-xs leading-relaxed text-slate-300">{log.message}</p>
                  </li>
                ))}
              </ul>
            </div>
          </motion.aside>
        )}
      </AnimatePresence>
    </section>
  );
}

export default ConsoleDrawer;
