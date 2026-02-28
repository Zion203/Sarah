import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Component, Suspense, lazy, useEffect, useMemo, useState, type ErrorInfo, type ReactNode } from "react";

import { useTheme } from "@/hooks/useTheme";
import "@/styles/sarah-ai.css";

import type { SetupState } from "@/components/SetupWindow";

const SetupWindow = lazy(() => import("@/components/SetupWindow"));
const MainOverlayApp = lazy(() => import("@/components/MainOverlayApp"));
const SettingsWindow = lazy(() => import("@/components/SettingsWindow"));
const HistoryWindow = lazy(() => import("@/components/HistoryWindow"));
const ModelsWindow = lazy(() => import("@/components/ModelsWindow"));
const McpMarketplaceWindow = lazy(() => import("@/components/McpMarketplaceWindow"));
const SpotifyAudioPlayer = lazy(() =>
  import("@/components/SpotifyAudioPlayer").then((module) => ({
    default: module.SpotifyAudioPlayer,
  })),
);
import { getCurrentWindow } from "@tauri-apps/api/window";

type WindowType = "main" | "settings" | "history" | "models" | "mcp" | "audio";

declare global {
  interface Window {
    __SARAH_WINDOW_TYPE__?: string;
  }
}

function normalizeWindowType(value: null | string | undefined): null | WindowType {
  const normalized = value?.trim().toLowerCase();
  if (
    normalized === "main" ||
    normalized === "settings" ||
    normalized === "history" ||
    normalized === "models" ||
    normalized === "mcp" ||
    normalized === "audio"
  ) {
    return normalized;
  }
  return null;
}

function resolveWindowType(): WindowType {
  if (typeof window === "undefined") {
    return "main";
  }

  const queryWindowType = normalizeWindowType(
    new URLSearchParams(window.location.search).get("window"),
  );
  if (queryWindowType) {
    if (import.meta.env.DEV) {
      console.debug(`[sarah.window] resolved "${queryWindowType}" from query string`);
    }
    return queryWindowType;
  }

  const injectedWindowType = normalizeWindowType(window.__SARAH_WINDOW_TYPE__);
  if (injectedWindowType) {
    if (import.meta.env.DEV) {
      console.debug(`[sarah.window] resolved "${injectedWindowType}" from init script`);
    }
    return injectedWindowType;
  }

  try {
    const labelWindowType = normalizeWindowType(getCurrentWindow().label);
    if (labelWindowType) {
      if (import.meta.env.DEV) {
        console.debug(`[sarah.window] resolved "${labelWindowType}" from Tauri label`);
      }
      return labelWindowType;
    }
  } catch {
    // Ignore when outside Tauri runtime.
  }

  if (import.meta.env.DEV) {
    console.debug('[sarah.window] resolved "main" from fallback');
  }
  return "main";
}

interface SecondaryWindowErrorBoundaryProps {
  children: ReactNode;
}

interface SecondaryWindowErrorBoundaryState {
  hasError: boolean;
  message: string;
}

class SecondaryWindowErrorBoundary extends Component<
  SecondaryWindowErrorBoundaryProps,
  SecondaryWindowErrorBoundaryState
> {
  state: SecondaryWindowErrorBoundaryState = {
    hasError: false,
    message: "",
  };

  static getDerivedStateFromError(error: unknown): SecondaryWindowErrorBoundaryState {
    const message =
      error instanceof Error
        ? error.message
        : typeof error === "string"
          ? error
          : "An unexpected error occurred while rendering this window.";
    return {
      hasError: true,
      message,
    };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    console.error("Secondary window render failure", error, info);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex h-screen w-screen items-center justify-center bg-background p-6 text-foreground">
          <div className="max-w-xl rounded-md border border-border bg-card p-4 text-sm">
            <p className="font-semibold">Window failed to render.</p>
            <p className="mt-2 text-muted-foreground">{this.state.message}</p>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

function App() {
  const windowType = useMemo(resolveWindowType, []);
  const { isDarkTheme, theme, toggleTheme } = useTheme();
  const [isBackendReady, setIsBackendReady] = useState(false);
  const [setupState, setSetupState] = useState<SetupState | null | undefined>(undefined);

  useEffect(() => {
    document.documentElement.setAttribute("data-window-type", windowType);
  }, [windowType]);

  useEffect(() => {
    // If it's a secondary window, we could assume backend is ready, or wait. Waiting is safer.
    const unlisten = listen("backend-ready", () => {
      setIsBackendReady(true);
    });

    // Also invoke a ping just in case the event already fired before we started listening
    invoke("get_startup_telemetry")
      .then(() => setIsBackendReady(true))
      .catch(() => { });

    invoke("get_setup_status")
      .then((res) => setSetupState((res as SetupState) || null))
      .catch(() => setSetupState(null));

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  // Secondary windows should render immediately without waiting for setup/backend
  if (windowType !== "main") {
    return (
      <SecondaryWindowErrorBoundary>
        <Suspense
          fallback={
            <div className="flex h-screen w-screen items-center justify-center bg-background text-foreground text-sm">
              Loading window...
            </div>
          }
        >
          {windowType === "settings" ? (
            <SettingsWindow onToggleTheme={toggleTheme} theme={theme} />
          ) : windowType === "history" ? (
            <HistoryWindow />
          ) : windowType === "models" ? (
            <ModelsWindow />
          ) : windowType === "mcp" ? (
            <McpMarketplaceWindow />
          ) : windowType === "audio" ? (
            <main className="sarah-audio-window" aria-label="Spotify audio window">
              <SpotifyAudioPlayer
                isOpen
                draggable={false}
                autoplayOnOpen
                windowTitlebarDrag
                onOpenChange={(open) => {
                  if (!open) {
                    void invoke("close_audio_window");
                  }
                }}
              />
            </main>
          ) : (
            <div className="flex h-screen w-screen items-center justify-center bg-background text-foreground text-sm">
              Unable to resolve window type.
            </div>
          )}
        </Suspense>
      </SecondaryWindowErrorBoundary>
    );
  }

  if (setupState === undefined) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-background text-foreground" data-tauri-drag-region>
        <div className="flex flex-col items-center gap-4">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          <p className="text-sm font-medium animate-pulse">Waking up Sarah AI...</p>
        </div>
      </div>
    );
  }

  // Eagerly show Setup Window if setup hasn't occurred, regardless of backend readiness
  if (setupState === null || setupState.status !== "completed") {
    return (
      <Suspense fallback={null}>
        <SetupWindow initialState={setupState || null} onComplete={() => setSetupState({ status: "completed" } as SetupState)} />
      </Suspense>
    );
  }

  // Wait for the backend to be fully loaded (models cached, RAG warmed) 
  // ONLY if setup is actually completed
  if (!isBackendReady) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-background text-foreground" data-tauri-drag-region>
        <div className="flex flex-col items-center gap-4">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          <p className="text-sm font-medium animate-pulse">Initializing Neural Engines...</p>
        </div>
      </div>
    );
  }

  return (
    <Suspense fallback={null}>
      <MainOverlayApp isDarkTheme={isDarkTheme} onToggleTheme={toggleTheme} />
    </Suspense>
  );
}

export default App;
