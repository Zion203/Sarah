import { invoke } from "@tauri-apps/api/core";
import { Suspense, lazy, useMemo } from "react";

import { useTheme } from "@/hooks/useTheme";
import "@/styles/sarah-ai.css";

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

function App() {
  const windowType = useMemo(
    () => new URLSearchParams(window.location.search).get("window") ?? "main",
    [],
  );
  const { isDarkTheme, theme, toggleTheme } = useTheme();

  return (
    <Suspense fallback={null}>
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
        <MainOverlayApp isDarkTheme={isDarkTheme} onToggleTheme={toggleTheme} />
      )}
    </Suspense>
  );
}

export default App;
