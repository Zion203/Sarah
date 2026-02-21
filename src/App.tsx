import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion } from "framer-motion";
import { ChevronDown, Store } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import AssistantInput from "@/components/AssistantInput";
import ConversationFeed from "@/components/ConversationFeed";
import HistoryWindow from "@/components/HistoryWindow";
import SettingsWindow from "@/components/SettingsWindow";
import { useTheme } from "@/hooks/useTheme";
import { useUIState } from "@/hooks/useUIState";
import "./styles/sarah-ai.css";

const WINDOW_WIDTH = 520;
const WINDOW_HEIGHT_HIDDEN = 88;
const WINDOW_HEIGHT_INPUT_ONLY = 102;
const WINDOW_HEIGHT_WITH_RESPONSE = 226;

interface MainOverlayAppProps {
  isDarkTheme: boolean;
  onToggleTheme: () => void;
}

function MainOverlayApp({ isDarkTheme, onToggleTheme }: MainOverlayAppProps) {
  const [isUiVisible, setIsUiVisible] = useState(false);
  const [isResponseVisible, setIsResponseVisible] = useState(true);
  const {
    amplitude,
    clearPrompt,
    conversations,
    cycleState,
    isPromptLocked,
    prompt,
    setPrompt,
    setState,
    state,
    stopResponse,
    submitPrompt,
  } = useUIState();

  const openHistoryWindow = useCallback(() => {
    void invoke("open_history_window").catch((error) => {
      console.error("Failed to open history window.", error);
    });
  }, []);

  const openSettingsWindow = useCallback(() => {
    void invoke("open_settings_window").catch((error) => {
      console.error("Failed to open settings window.", error);
    });
  }, []);

  const showStopAction = isPromptLocked;

  const handleSubmit = useCallback(() => {
    if (prompt.trim().toLowerCase() === "/history") {
      clearPrompt();
      openHistoryWindow();
      return;
    }

    setIsResponseVisible(true);
    submitPrompt();
  }, [clearPrompt, openHistoryWindow, prompt, submitPrompt]);

  const handleStopAction = useCallback(() => {
    stopResponse();
  }, [stopResponse]);

  const handleOpenMcpMarketplace = useCallback(() => {
    console.log("MCP Marketplace clicked. Integrate marketplace window here.");
  }, []);

  useEffect(() => {
    const targetHeight = isUiVisible
      ? isResponseVisible
        ? WINDOW_HEIGHT_WITH_RESPONSE
        : WINDOW_HEIGHT_INPUT_ONLY
      : WINDOW_HEIGHT_HIDDEN;

    void getCurrentWindow()
      .setSize(new LogicalSize(WINDOW_WIDTH, targetHeight))
      .catch(() => {
        // Ignore if not running in Tauri context.
      });
  }, [isResponseVisible, isUiVisible]);

  useEffect(() => {
    let unlisten: null | (() => void) = null;
    let disposed = false;

    void listen("sarah://show-overlay", () => {
      setIsUiVisible(true);
    })
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
      })
      .catch(() => {
        // Ignore if not running in Tauri context.
      });

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const isTypingTarget =
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        target?.isContentEditable;

      if (event.repeat) {
        return;
      }

      if (event.ctrlKey && event.code === "Space") {
        event.preventDefault();
        setIsUiVisible((current) => !current);
        return;
      }

      if (isUiVisible && !event.ctrlKey && event.code === "Space" && !isTypingTarget) {
        event.preventDefault();
        cycleState();
        return;
      }

      if (isUiVisible && event.code === "Escape") {
        event.preventDefault();
        clearPrompt();
        setState("idle");
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [clearPrompt, cycleState, isUiVisible, setState]);

  return (
    <main className="sarah-overlay-root" aria-label="Sarah AI overlay root">
      <AnimatePresence>
        {isUiVisible && (
          <motion.section
            key="sarah-overlay"
            className="sarah-overlay-shell"
            initial={{ opacity: 0, y: -16, scale: 0.985 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -12, scale: 0.985 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            aria-label="Sarah AI compact input bar"
          >
            <AssistantInput
              amplitude={amplitude}
              isDarkTheme={isDarkTheme}
              onClear={clearPrompt}
              onOpenSettings={openSettingsWindow}
              onPromptChange={setPrompt}
              onStop={handleStopAction}
              onSubmit={handleSubmit}
              onToggleTheme={onToggleTheme}
              prompt={prompt}
              readOnly={isPromptLocked}
              showStopAction={showStopAction}
              state={state}
            />
            <div className="sarah-response-toolbar" data-tauri-disable-drag-region="true">
              <div className="sarah-response-toolbar__actions">
                <button
                  type="button"
                  className="sarah-response-toggle-button"
                  onClick={() => setIsResponseVisible((current) => !current)}
                  aria-expanded={isResponseVisible}
                  aria-controls="sarah-response-body"
                  title={isResponseVisible ? "Collapse response" : "Expand response"}
                >
                  <motion.span
                    animate={{ rotate: isResponseVisible ? 0 : -180 }}
                    transition={{ duration: 0.2, ease: "easeOut" }}
                  >
                    <ChevronDown className="size-3.5" />
                  </motion.span>
                </button>
                <button
                  type="button"
                  className="sarah-mcp-marketplace-button"
                  onClick={handleOpenMcpMarketplace}
                  title="MCP Marketplace"
                >
                  <Store className="size-3.5" />
                  <span>MCP Marketplace</span>
                </button>
              </div>
            </div>

            <AnimatePresence initial={false}>
              {isResponseVisible && (
                <motion.div
                  id="sarah-response-body"
                  className="sarah-response-collapse"
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: "auto", opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: 0.22, ease: [0.32, 0.72, 0, 1] }}
                >
                  <ConversationFeed items={conversations} />
                </motion.div>
              )}
            </AnimatePresence>
          </motion.section>
        )}
      </AnimatePresence>
    </main>
  );
}

function App() {
  const windowType = useMemo(
    () => new URLSearchParams(window.location.search).get("window") ?? "main",
    [],
  );
  const { isDarkTheme, theme, toggleTheme } = useTheme();

  if (windowType === "settings") {
    return <SettingsWindow onToggleTheme={toggleTheme} theme={theme} />;
  }

  if (windowType === "history") {
    return <HistoryWindow />;
  }

  return <MainOverlayApp isDarkTheme={isDarkTheme} onToggleTheme={toggleTheme} />;
}

export default App;
