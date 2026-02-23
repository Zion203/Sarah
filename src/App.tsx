import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion } from "framer-motion";
import { ChevronDown, Store } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import AssistantInput from "@/components/AssistantInput";
import ConversationFeed from "@/components/ConversationFeed";
import HistoryWindow from "@/components/HistoryWindow";
import ScreenRecordingHud from "@/components/ScreenRecordingHud";
import SettingsWindow from "@/components/SettingsWindow";
import { useAppPreferences } from "@/hooks/useAppPreferences";
import { useScreenRecording } from "@/hooks/useScreenRecording";
import { useTheme } from "@/hooks/useTheme";
import { useUIState } from "@/hooks/useUIState";
import type { DesktopWindowSource } from "@/types/screenSources";
import "./styles/sarah-ai.css";

const WINDOW_WIDTH = 520;
const WINDOW_HEIGHT_HIDDEN = 88;
const WINDOW_HEIGHT_INPUT_ONLY = 102;
const WINDOW_HEIGHT_WITH_RESPONSE = 226;
type CaptureIntent = "record" | "take";

interface SlashCommandDefinition {
  command: string;
  description: string;
  searchTerms: string[];
}

const AVAILABLE_SLASH_COMMANDS: SlashCommandDefinition[] = [
  {
    command: "/history",
    description: "Open your local chat history window.",
    searchTerms: ["history", "chat", "past"],
  },
  {
    command: "/record",
    description: "Start screen recording using your default capture source.",
    searchTerms: ["record", "video", "capture", "start"],
  },
  {
    command: "/record start",
    description: "Explicitly start a new screen recording session.",
    searchTerms: ["record", "video", "capture", "start"],
  },
  {
    command: "/record stop",
    description: "Stop the active recording session.",
    searchTerms: ["record", "stop", "end"],
  },
  {
    command: "/take",
    description: "Take a screenshot using your default capture source.",
    searchTerms: ["take", "screenshot", "screen", "capture"],
  },
  {
    command: "/take screenshot",
    description: "Explicitly capture a screenshot.",
    searchTerms: ["take", "screenshot", "capture"],
  },
];

interface NativeScreenshotResult {
  capturedAtMs: number;
  screenshotPath: string;
}

function formatRecordingDuration(durationMs: number) {
  const totalSeconds = Math.max(0, Math.floor(durationMs / 1000));
  const minutes = Math.floor(totalSeconds / 60)
    .toString()
    .padStart(2, "0");
  const seconds = (totalSeconds % 60).toString().padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function normalizeDesktopWindowSources(value: unknown): DesktopWindowSource[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((item) => {
      if (typeof item !== "object" || item === null) {
        return null;
      }

      const row = item as Record<string, unknown>;
      const id =
        typeof row.id === "string"
          ? row.id.trim()
          : typeof row.id === "number" && Number.isFinite(row.id)
            ? String(Math.trunc(row.id))
            : "";
      const processName = typeof row.processName === "string" ? row.processName : "";
      const title = typeof row.title === "string" ? row.title.trim() : "";

      if (!id || !title) {
        return null;
      }

      return {
        id,
        processName: processName || "Unknown app",
        title,
      };
    })
    .filter((item): item is DesktopWindowSource => item !== null);
}

interface MainOverlayAppProps {
  isDarkTheme: boolean;
  onToggleTheme: () => void;
}

function MainOverlayApp({ isDarkTheme, onToggleTheme }: MainOverlayAppProps) {
  const consumedRecordingIdRef = useRef<string | null>(null);
  const isRecordingTransitionRef = useRef(false);
  const uiVisibleBeforeRecordingRef = useRef(false);
  const [isUiVisible, setIsUiVisible] = useState(false);
  const [isResponseVisible, setIsResponseVisible] = useState(true);
  const [isWindowSourceSelectionVisible, setIsWindowSourceSelectionVisible] = useState(false);
  const [isWindowSourceSelectionLoading, setIsWindowSourceSelectionLoading] = useState(false);
  const [windowSourceSelectionError, setWindowSourceSelectionError] = useState<null | string>(null);
  const [windowSourceSelectionItems, setWindowSourceSelectionItems] = useState<DesktopWindowSource[]>([]);
  const [pendingCaptureIntent, setPendingCaptureIntent] = useState<CaptureIntent | null>(null);
  const [selectedWindowTitle, setSelectedWindowTitle] = useState<null | string>(null);
  const {
    amplitude,
    clearPrompt,
    conversations,
    cycleState,
    isPromptLocked,
    prompt,
    setPrompt,
    setSystemConversation,
    setState,
    state,
    stopResponse,
    submitPrompt,
  } = useUIState();
  const { preferences, updatePreferences } = useAppPreferences();
  const {
    clearError: clearScreenRecordingError,
    elapsedMs: screenElapsedMs,
    isRecording: isScreenRecording,
    lastError: screenRecordingError,
    result: screenRecordingResult,
    startRecording: startScreenRecording,
    stopRecording: stopScreenRecording,
  } = useScreenRecording();

  const openHistoryWindow = useCallback(() => {
    void invoke("open_history_window").catch((error) => {
      console.error("Failed to open history window.", error);
    });
  }, []);

  const openSettingsWindow = useCallback(() => {
    setIsUiVisible(false);
    void invoke("open_settings_window").catch((error) => {
      console.error("Failed to open settings window.", error);
    });
  }, []);

  const showStopAction = isPromptLocked || isScreenRecording;
  const captureOutputDirectory = preferences.captureOutputDirectory ?? undefined;
  const normalizedPrompt = useMemo(() => prompt.trim().toLowerCase(), [prompt]);
  const showSlashCommands = !isPromptLocked && normalizedPrompt.startsWith("/");
  const slashCommandQuery = showSlashCommands ? normalizedPrompt.slice(1).trim() : "";
  const filteredSlashCommands = useMemo(() => {
    if (!showSlashCommands) {
      return [];
    }

    if (!slashCommandQuery) {
      return AVAILABLE_SLASH_COMMANDS;
    }

    return AVAILABLE_SLASH_COMMANDS.filter((item) => {
      const normalizedCommand = item.command.slice(1).toLowerCase();
      if (normalizedCommand.includes(slashCommandQuery)) {
        return true;
      }

      return item.searchTerms.some((term) => term.includes(slashCommandQuery));
    });
  }, [showSlashCommands, slashCommandQuery]);

  const intentCommandLabel = useCallback((intent: CaptureIntent) => {
    return intent === "record" ? "/record" : "/take";
  }, []);

  const markSurfacePermission = useCallback(
    (surface: "screen" | "window", granted: boolean) => {
      updatePreferences((current) => ({
        ...current,
        screenPermissions: {
          ...current.screenPermissions,
          [surface]: granted,
        },
        screenPermissionGrantedAt: granted
          ? {
              ...current.screenPermissionGrantedAt,
              [surface]: new Date().toISOString(),
            }
          : current.screenPermissionGrantedAt,
      }));
    },
    [updatePreferences],
  );

  const startRecordingWithSurface = useCallback(
    (surface: "screen" | "window", sourceName?: string, windowHwnd?: string) => {
      void (async () => {
        const startResult = await startScreenRecording(surface, windowHwnd, captureOutputDirectory);
        setIsResponseVisible(true);

        if (startResult.ok) {
          markSurfacePermission(surface, true);
          const label =
            surface === "window"
              ? sourceName
                ? `Window capture started for "${sourceName}".`
                : "Window capture started."
              : "Entire screen capture started.";
          setSystemConversation("/record", `${label} Use the floating stop button to finish.`);
          return;
        }

        if (startResult.error?.toLowerCase().includes("denied")) {
          markSurfacePermission(surface, false);
        }

        if (!startResult.error) {
          setSystemConversation("/record", "Unable to start screen capture.");
        }
      })();
    },
    [captureOutputDirectory, markSurfacePermission, setSystemConversation, startScreenRecording],
  );

  const takeScreenshotWithSurface = useCallback(
    (surface: "screen" | "window", sourceName?: string, windowHwnd?: string) => {
      void (async () => {
        setIsResponseVisible(true);
        try {
          const payload: Record<string, unknown> = {
            surface,
          };
          if (windowHwnd) {
            payload.windowHwnd = windowHwnd;
          }
          if (captureOutputDirectory) {
            payload.outputDirectory = captureOutputDirectory;
          }

          const result = await invoke<NativeScreenshotResult>("take_native_screenshot", payload);
          markSurfacePermission(surface, true);
          const targetLabel =
            surface === "window"
              ? sourceName
                ? ` for "${sourceName}"`
                : " for selected window"
              : "";
          setSystemConversation(
            "/take",
            `Screenshot captured${targetLabel}. Saved to ${result.screenshotPath}.`,
            true,
          );
          setSelectedWindowTitle(null);
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : typeof error === "object" &&
                  error !== null &&
                  "message" in error &&
                  typeof (error as { message: unknown }).message === "string"
                ? (error as { message: string }).message
                : "Screenshot failed.";

          if (message.toLowerCase().includes("denied")) {
            markSurfacePermission(surface, false);
          }
          setSystemConversation("/take", message);
        }
      })();
    },
    [captureOutputDirectory, markSurfacePermission, setSystemConversation],
  );

  const loadWindowSources = useCallback((intent: CaptureIntent) => {
    void (async () => {
      setIsResponseVisible(true);
      setIsWindowSourceSelectionVisible(true);
      setIsWindowSourceSelectionLoading(true);
      setWindowSourceSelectionError(null);
      setWindowSourceSelectionItems([]);
      setPendingCaptureIntent(intent);
      setSelectedWindowTitle(null);
      setSystemConversation(
        intentCommandLabel(intent),
        intent === "record"
          ? "Select an active window below to record."
          : "Select an active window below to capture screenshot.",
      );

      try {
        const raw = await invoke<unknown>("list_active_windows");
        const sources = normalizeDesktopWindowSources(raw);
        setWindowSourceSelectionItems(sources);
      } catch (error) {
        const message =
          error instanceof Error
            ? error.message
            : typeof error === "object" &&
                error !== null &&
                "message" in error &&
                typeof (error as { message: unknown }).message === "string"
              ? (error as { message: string }).message
              : "Failed to load active windows.";
        setWindowSourceSelectionError(message);
      } finally {
        setIsWindowSourceSelectionLoading(false);
      }
    })();
  }, [intentCommandLabel, setSystemConversation]);

  const runCaptureIntent = useCallback((intent: CaptureIntent) => {
    if (!preferences.allowScreenRecording) {
      setIsResponseVisible(true);
      setSystemConversation(
        intentCommandLabel(intent),
        "Read my screen is disabled. Enable it in Settings > Permissions.",
      );
      return;
    }

    const preferredSurface = preferences.screenCaptureSurface;
    if (!preferences.screenPermissions[preferredSurface]) {
      setIsResponseVisible(true);
      setSystemConversation(
        intentCommandLabel(intent),
        `${preferredSurface === "window" ? "Window" : "Entire screen"} permission is missing. Grant it in Settings > Permissions.`,
      );
      return;
    }

    if (preferredSurface === "window") {
      loadWindowSources(intent);
      return;
    }

    setIsWindowSourceSelectionVisible(false);
    setWindowSourceSelectionItems([]);
    setWindowSourceSelectionError(null);
    setPendingCaptureIntent(null);
    setSelectedWindowTitle(null);
    if (intent === "record") {
      startRecordingWithSurface("screen");
      return;
    }
    takeScreenshotWithSurface("screen");
  }, [
    intentCommandLabel,
    loadWindowSources,
    preferences,
    setSystemConversation,
    startRecordingWithSurface,
    takeScreenshotWithSurface,
  ]);

  const handleStopRecordCommand = useCallback(() => {
    if (!isScreenRecording) {
      setSystemConversation("/record", "No active recording to stop.");
      return;
    }

    setIsWindowSourceSelectionVisible(false);
    setPendingCaptureIntent(null);
    stopScreenRecording();
  }, [isScreenRecording, setSystemConversation, stopScreenRecording]);

  const handleWindowSourceSelect = useCallback(
    (source: DesktopWindowSource) => {
      setSelectedWindowTitle(source.title);
      setIsWindowSourceSelectionVisible(false);
      const intent = pendingCaptureIntent ?? "record";
      setPendingCaptureIntent(null);
      if (intent === "record") {
        startRecordingWithSurface("window", source.title, source.id);
        return;
      }
      takeScreenshotWithSurface("window", source.title, source.id);
    },
    [pendingCaptureIntent, startRecordingWithSurface, takeScreenshotWithSurface],
  );

  const handleSubmit = useCallback(() => {
    const normalizedPromptValue = prompt.trim().toLowerCase();

    if (normalizedPromptValue === "/history") {
      setIsWindowSourceSelectionVisible(false);
      setPendingCaptureIntent(null);
      clearPrompt();
      openHistoryWindow();
      return;
    }

    if (normalizedPromptValue === "/record" || normalizedPromptValue === "/record start") {
      clearPrompt();
      runCaptureIntent("record");
      return;
    }

    if (normalizedPromptValue === "/record stop") {
      clearPrompt();
      handleStopRecordCommand();
      return;
    }

    if (normalizedPromptValue === "/take" || normalizedPromptValue === "/take screenshot") {
      clearPrompt();
      runCaptureIntent("take");
      return;
    }

    if (normalizedPromptValue.startsWith("/screen")) {
      clearPrompt();
      setSystemConversation("/record", 'Use `/record` for video and `/take` for screenshot.');
      return;
    }

    setIsWindowSourceSelectionVisible(false);
    setWindowSourceSelectionItems([]);
    setWindowSourceSelectionError(null);
    setPendingCaptureIntent(null);
    setSelectedWindowTitle(null);
    setIsResponseVisible(true);
    submitPrompt();
  }, [
    clearPrompt,
    handleStopRecordCommand,
    openHistoryWindow,
    prompt,
    runCaptureIntent,
    setSystemConversation,
    submitPrompt,
  ]);

  const handleStopAction = useCallback(() => {
    if (isScreenRecording) {
      setIsWindowSourceSelectionVisible(false);
      setPendingCaptureIntent(null);
      stopScreenRecording();
      return;
    }

    stopResponse();
  }, [isScreenRecording, stopResponse, stopScreenRecording]);

  const handleOpenMcpMarketplace = useCallback(() => {
    console.log("MCP Marketplace clicked. Integrate marketplace window here.");
  }, []);

  useEffect(() => {
    if (!screenRecordingResult) {
      return;
    }

    if (consumedRecordingIdRef.current === screenRecordingResult.id) {
      return;
    }
    consumedRecordingIdRef.current = screenRecordingResult.id;

    const videoPath = screenRecordingResult.videoPath;
    const screenshotPath = screenRecordingResult.screenshotPath;
    const captureNotice = screenshotPath
      ? `Saved recording to ${videoPath} and screenshot to ${screenshotPath}.`
      : `Saved recording to ${videoPath}.`;

    setIsResponseVisible(true);
    setIsWindowSourceSelectionVisible(false);
    setWindowSourceSelectionItems([]);
    setWindowSourceSelectionError(null);
    setPendingCaptureIntent(null);
    setSystemConversation(
      "/record",
      `Screen recording complete${selectedWindowTitle ? ` for "${selectedWindowTitle}"` : ""} (${formatRecordingDuration(screenRecordingResult.durationMs)}). ${captureNotice}`,
      true,
    );
    setSelectedWindowTitle(null);
  }, [screenRecordingResult, selectedWindowTitle, setSystemConversation]);

  useEffect(() => {
    if (!screenRecordingError) {
      return;
    }

    if (screenRecordingError.toLowerCase().includes("denied")) {
      const surface = preferences.screenCaptureSurface;
      markSurfacePermission(surface, false);
    }

    setIsResponseVisible(true);
    setSystemConversation("/record", screenRecordingError);
    clearScreenRecordingError();
    setIsWindowSourceSelectionVisible(false);
    setWindowSourceSelectionItems([]);
    setWindowSourceSelectionError(null);
    setPendingCaptureIntent(null);
    setSelectedWindowTitle(null);
  }, [
    clearScreenRecordingError,
    markSurfacePermission,
    preferences.screenCaptureSurface,
    screenRecordingError,
    setSystemConversation,
  ]);

  useEffect(() => {
    void (async () => {
      try {
        const currentWindow = getCurrentWindow();

        if (isScreenRecording) {
          await currentWindow.maximize();
          return;
        }

        if (await currentWindow.isMaximized()) {
          await currentWindow.unmaximize();
        }

        const targetHeight = isUiVisible
          ? isResponseVisible
            ? WINDOW_HEIGHT_WITH_RESPONSE
            : WINDOW_HEIGHT_INPUT_ONLY
          : WINDOW_HEIGHT_HIDDEN;

        await currentWindow.setSize(new LogicalSize(WINDOW_WIDTH, targetHeight));
      } catch {
        // Ignore if not running in Tauri context.
      }
    })();
  }, [isResponseVisible, isScreenRecording, isUiVisible]);

  useEffect(() => {
    if (isScreenRecording && !isRecordingTransitionRef.current) {
      uiVisibleBeforeRecordingRef.current = isUiVisible;
      setIsUiVisible(false);
      isRecordingTransitionRef.current = true;
      return;
    }

    if (!isScreenRecording && isRecordingTransitionRef.current) {
      setIsUiVisible(uiVisibleBeforeRecordingRef.current);
      isRecordingTransitionRef.current = false;
    }
  }, [isScreenRecording, isUiVisible]);

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
        if (isScreenRecording) {
          return;
        }
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
        if (isWindowSourceSelectionVisible) {
          setIsWindowSourceSelectionVisible(false);
          setIsResponseVisible(true);
          return;
        }
        clearPrompt();
        setState("idle");
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    clearPrompt,
    cycleState,
    isScreenRecording,
    isUiVisible,
    isWindowSourceSelectionVisible,
    setState,
  ]);

  useEffect(() => {
    if (showSlashCommands) {
      setIsResponseVisible(true);
    }
  }, [showSlashCommands]);

  return (
    <main
      className={`sarah-overlay-root ${isScreenRecording ? "sarah-overlay-root--screen-recording" : ""}`}
      aria-label="Sarah AI overlay root"
    >
      <AnimatePresence>
        {isScreenRecording && (
          <motion.div
            key="sarah-screen-scan"
            className="sarah-screen-scan-overlay"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            aria-hidden="true"
          />
        )}
      </AnimatePresence>
      <ScreenRecordingHud
        elapsedMs={screenElapsedMs}
        isVisible={isScreenRecording}
        onStop={handleStopRecordCommand}
      />
      <AnimatePresence>
        {isUiVisible && !isScreenRecording && (
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
                  <ConversationFeed
                    items={conversations}
                    isWindowSourceSelection={isWindowSourceSelectionVisible}
                    onWindowSourceSelect={handleWindowSourceSelect}
                    showSlashCommands={showSlashCommands}
                    slashCommandQuery={slashCommandQuery}
                    slashCommands={filteredSlashCommands}
                    windowSourceError={windowSourceSelectionError}
                    windowSourceLoading={isWindowSourceSelectionLoading}
                    windowSources={windowSourceSelectionItems}
                  />
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
