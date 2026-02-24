import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";
import {
  BadgeCheck,
  CirclePause,
  FolderOpen,
  Loader2,  
  Minus,
  Music2,
  Plug,
  Server,
  ShieldCheck,
  Sparkles,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";

const STORAGE_KEY = "sarah_spotify_mcp_config_v1";
const DEFAULT_SERVER_ROOT =
  "C:\\Users\\jesud\\OneDrive\\Desktop\\personal\\Sarah\\mcp\\spotify-mcp-server";
const DRAG_BLOCK_SELECTOR = [
  "button",
  "input",
  "textarea",
  "select",
  "option",
  "a",
  "label",
  "[role='button']",
  "[data-tauri-disable-drag-region='true']",
  "[contenteditable='true']",
].join(",");

type SpotifyMcpConfig = {
  serverRoot: string;
  clientId: string;
  clientSecret: string;
  redirectUri: string;
  autoStart: boolean;
};

const DEFAULT_CONFIG: SpotifyMcpConfig = {
  serverRoot: DEFAULT_SERVER_ROOT,
  clientId: "",
  clientSecret: "",
  redirectUri: "http://127.0.0.1:8888/callback",
  autoStart: false,
};

function readConfig(): SpotifyMcpConfig {
  if (typeof window === "undefined") {
    return DEFAULT_CONFIG;
  }

  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return DEFAULT_CONFIG;
    }
    const parsed = JSON.parse(raw) as Partial<SpotifyMcpConfig>;
    return {
      ...DEFAULT_CONFIG,
      ...parsed,
    };
  } catch {
    return DEFAULT_CONFIG;
  }
}

function writeConfig(config: SpotifyMcpConfig) {
  if (typeof window === "undefined") {
    return;
  }
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
}

function McpMarketplaceWindow() {
  const [config, setConfig] = useState<SpotifyMcpConfig>(() => readConfig());
  const [isRunning, setIsRunning] = useState(false);
  const [statusMessage, setStatusMessage] = useState("Spotify MCP is offline.");
  const [isWorking, setIsWorking] = useState(false);
  const [isAuthWorking, setIsAuthWorking] = useState(false);
  const [isBuildWorking, setIsBuildWorking] = useState(false);
  const [isSavingConfig, setIsSavingConfig] = useState(false);
  const pendingRef = useRef(false);
  const autoStartAttemptedRef = useRef(false);

  useEffect(() => {
    writeConfig(config);
  }, [config]);

  const refreshStatus = useCallback(async () => {
    try {
      const running = await invoke<boolean>("spotify_mcp_status");
      setIsRunning(running);
      setStatusMessage(
        running ? "Spotify MCP is running and ready." : "Spotify MCP is offline.",
      );
    } catch (error) {
      console.error("Failed to read Spotify MCP status.", error);
      setIsRunning(false);
      setStatusMessage("Could not reach the local Spotify MCP process.");
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const handleStart = useCallback(async () => {
    if (pendingRef.current) {
      return;
    }
    pendingRef.current = true;
    setIsWorking(true);

    try {
      const entryPath = await join(config.serverRoot, "build", "index.js");
      await invoke("start_spotify_mcp", { entryPath });
      setIsRunning(true);
      setStatusMessage("Spotify MCP is running and ready.");
    } catch (error) {
      console.error("Failed to start Spotify MCP server.", error);
      const message =
        error instanceof Error ? error.message : "Failed to start Spotify MCP server.";
      setStatusMessage(message);
      setIsRunning(false);
    } finally {
      pendingRef.current = false;
      setIsWorking(false);
    }
  }, [config.serverRoot]);

  const handleStop = useCallback(async () => {
    if (pendingRef.current) {
      return;
    }
    pendingRef.current = true;
    setIsWorking(true);

    try {
      await invoke("stop_spotify_mcp");
      setIsRunning(false);
      setStatusMessage("Spotify MCP is offline.");
    } catch (error) {
      console.error("Failed to stop Spotify MCP server.", error);
      setStatusMessage("Failed to stop Spotify MCP server.");
    } finally {
      pendingRef.current = false;
      setIsWorking(false);
    }
  }, []);

  const handleRunOAuth = useCallback(async () => {
    if (isAuthWorking) {
      return;
    }

    setIsAuthWorking(true);
    try {
      await invoke("run_spotify_oauth", { serverRoot: config.serverRoot });
      setStatusMessage("OAuth completed. Tokens stored in spotify-config.json.");
    } catch (error) {
      console.error("Failed to run Spotify OAuth.", error);
      const message =
        error instanceof Error ? error.message : "Failed to run Spotify OAuth.";
      setStatusMessage(message);
    } finally {
      setIsAuthWorking(false);
    }
  }, [config.serverRoot, isAuthWorking]);

  const handleBuildServer = useCallback(async () => {
    if (isBuildWorking) {
      return;
    }

    setIsBuildWorking(true);
    try {
      await invoke("build_spotify_mcp", { serverRoot: config.serverRoot });
      setStatusMessage("Spotify MCP built successfully.");
    } catch (error) {
      console.error("Failed to build Spotify MCP.", error);
      const message =
        error instanceof Error ? error.message : "Failed to build Spotify MCP.";
      setStatusMessage(message);
    } finally {
      setIsBuildWorking(false);
    }
  }, [config.serverRoot, isBuildWorking]);

  const handleSaveConfig = useCallback(async () => {
    if (isSavingConfig) {
      return;
    }

    setIsSavingConfig(true);
    try {
      await invoke("write_spotify_config", {
        serverRoot: config.serverRoot,
        clientId: config.clientId,
        clientSecret: config.clientSecret,
        redirectUri: config.redirectUri,
      });
      setStatusMessage("spotify-config.json updated.");
    } catch (error) {
      console.error("Failed to save Spotify config.", error);
      const message =
        error instanceof Error ? error.message : "Failed to save Spotify config.";
      setStatusMessage(message);
    } finally {
      setIsSavingConfig(false);
    }
  }, [config.clientId, config.clientSecret, config.redirectUri, config.serverRoot, isSavingConfig]);

  useEffect(() => {
    if (!config.autoStart || isRunning || isWorking || autoStartAttemptedRef.current) {
      return;
    }

    autoStartAttemptedRef.current = true;
    void handleStart();
  }, [config.autoStart, handleStart, isRunning, isWorking]);

  const handleClose = async () => {
    try {
      await getCurrentWindow().close();
    } catch (error) {
      console.error("Failed to close MCP window.", error);
    }
  };

  const handleMinimize = async () => {
    try {
      await getCurrentWindow().minimize();
    } catch (error) {
      console.error("Failed to minimize MCP window.", error);
    }
  };

  const statusTone = useMemo(() => (isRunning ? "live" : "idle"), [isRunning]);

  const handleWindowMouseDownCapture = async (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0 || event.detail > 1) {
      return;
    }

    const target = event.target as HTMLElement;
    if (target.closest(DRAG_BLOCK_SELECTOR)) {
      return;
    }

    try {
      await getCurrentWindow().startDragging();
    } catch (error) {
      console.error("Failed to start dragging MCP window.", error);
    }
  };

  return (
    <main
      className="sarah-mcp-window"
      aria-label="Sarah AI MCP marketplace"
      onMouseDownCapture={handleWindowMouseDownCapture}
    >
      <section className="sarah-mcp-shell">
        <header
          className="sarah-mcp-titlebar"
          onDoubleClickCapture={(event) => {
            event.preventDefault();
            event.stopPropagation();
          }}
        >
          <div className="sarah-mcp-titlebar__meta">MCP Marketplace</div>
          <p className="sarah-mcp-titlebar__title">Sarah AI MCP Hub</p>
          <div className="sarah-mcp-titlebar__window-controls" data-tauri-disable-drag-region="true">
            <button
              type="button"
              className="sarah-mcp-titlebar__window-btn"
              aria-label="Minimize MCP window"
              data-tauri-disable-drag-region="true"
              onClick={() => void handleMinimize()}
            >
              <Minus className="size-3.5" />
            </button>
            <button
              type="button"
              className="sarah-mcp-titlebar__window-btn sarah-mcp-titlebar__window-btn--close"
              aria-label="Close MCP window"
              data-tauri-disable-drag-region="true"
              onClick={() => void handleClose()}
            >
              <X className="size-3.5" />
            </button>
          </div>
        </header>

        <div className="sarah-mcp-layout">
          <aside className="sarah-mcp-sidebar">
            <p className="sarah-mcp-sidebar__eyebrow">Integrations</p>
            <div className="sarah-mcp-sidebar__card">
              <div className="sarah-mcp-sidebar__icon">
                <Music2 className="size-4" />
              </div>
              <div>
                <p className="sarah-mcp-sidebar__title">Spotify MCP</p>
                <p className="sarah-mcp-sidebar__subtitle">Playback + playlist tools</p>
              </div>
            </div>
            <div className="sarah-mcp-sidebar__footnote">
              <ShieldCheck className="size-3.5" />
              <span>Local-only credentials and playback.</span>
            </div>
          </aside>

          <section className="sarah-mcp-content">
            <header className="sarah-mcp-hero">
              <p className="sarah-mcp-hero__eyebrow">Installed MCP</p>
              <div className="sarah-mcp-hero__title-row">
                <h1 className="sarah-mcp-hero__title">Spotify Control Suite</h1>
                <span className={`sarah-mcp-status sarah-mcp-status--${statusTone}`}>
                  {isRunning ? "Running" : "Offline"}
                </span>
              </div>
              <p className="sarah-mcp-hero__subtitle">
                Manage Spotify playback, queue, and playlists from Sarah. Make sure the MCP server
                is built before launching.
              </p>
            </header>

            <section className="sarah-mcp-grid">
              <article className="sarah-mcp-card">
                <header className="sarah-mcp-card__header">
                  <div>
                    <p className="sarah-mcp-card__title">Server status</p>
                    <p className="sarah-mcp-card__subtitle">{statusMessage}</p>
                  </div>
                  <div className="sarah-mcp-card__badge">
                    <Server className="size-4" />
                  </div>
                </header>
                <div className="sarah-mcp-card__actions">
                  <Button
                    type="button"
                    className="sarah-mcp-primary"
                    onClick={() => void handleStart()}
                    disabled={isWorking || isRunning}
                  >
                    {isWorking && !isRunning ? <Loader2 className="size-4 animate-spin" /> : null}
                    Start server
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    className="sarah-mcp-outline"
                    onClick={() => void handleStop()}
                    disabled={isWorking || !isRunning}
                  >
                    <CirclePause className="size-4" />
                    Stop server
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    className="sarah-mcp-outline"
                    onClick={() => void handleBuildServer()}
                    disabled={isBuildWorking}
                  >
                    {isBuildWorking ? <Loader2 className="size-4 animate-spin" /> : null}
                    Build server
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    className="sarah-mcp-outline"
                    onClick={() => void handleRunOAuth()}
                    disabled={isAuthWorking}
                  >
                    {isAuthWorking ? <Loader2 className="size-4 animate-spin" /> : null}
                    Run OAuth
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    className="sarah-mcp-ghost"
                    onClick={() => void refreshStatus()}
                    disabled={isWorking || isAuthWorking || isBuildWorking || isSavingConfig}
                  >
                    Refresh
                  </Button>
                </div>
              </article>

              <article className="sarah-mcp-card">
                <header className="sarah-mcp-card__header">
                  <div>
                    <p className="sarah-mcp-card__title">Server location</p>
                    <p className="sarah-mcp-card__subtitle">
                      Point to the Spotify MCP root folder (contains `package.json`).
                    </p>
                  </div>
                  <div className="sarah-mcp-card__badge">
                    <FolderOpen className="size-4" />
                  </div>
                </header>
                <div className="sarah-mcp-card__body">
                  <Input
                    value={config.serverRoot}
                    onChange={(event) =>
                      setConfig((current) => ({ ...current, serverRoot: event.target.value }))
                    }
                    placeholder={DEFAULT_SERVER_ROOT}
                    aria-label="Spotify MCP server root"
                  />
                  <div className="sarah-mcp-card__hint">
                    Build once with `npm install` and `npm run build` inside the server folder.
                  </div>
                </div>
              </article>

              <article className="sarah-mcp-card">
                <header className="sarah-mcp-card__header">
                  <div>
                    <p className="sarah-mcp-card__title">Spotify API credentials</p>
                    <p className="sarah-mcp-card__subtitle">
                      These values should match the `spotify-config.json` in the server folder.
                    </p>
                  </div>
                  <div className="sarah-mcp-card__badge">
                    <BadgeCheck className="size-4" />
                  </div>
                </header>
                <div className="sarah-mcp-card__body sarah-mcp-card__body--stack">
                  <div>
                    <label className="sarah-mcp-label">Client ID</label>
                    <Input
                      value={config.clientId}
                      onChange={(event) =>
                        setConfig((current) => ({ ...current, clientId: event.target.value }))
                      }
                      placeholder="Spotify client id"
                    />
                  </div>
                  <div>
                    <label className="sarah-mcp-label">Client secret</label>
                    <Input
                      type="password"
                      value={config.clientSecret}
                      onChange={(event) =>
                        setConfig((current) => ({ ...current, clientSecret: event.target.value }))
                      }
                      placeholder="Spotify client secret"
                    />
                  </div>
                  <div>
                    <label className="sarah-mcp-label">Redirect URI</label>
                    <Input
                      value={config.redirectUri}
                      onChange={(event) =>
                        setConfig((current) => ({ ...current, redirectUri: event.target.value }))
                      }
                      placeholder="http://127.0.0.1:8888/callback"
                    />
                  </div>
                  <div className="sarah-mcp-credentials-actions">
                    <Button
                      type="button"
                      variant="outline"
                      className="sarah-mcp-outline"
                      onClick={() => void handleSaveConfig()}
                      disabled={isSavingConfig}
                    >
                      {isSavingConfig ? <Loader2 className="size-4 animate-spin" /> : null}
                      Save config
                    </Button>
                    <p className="sarah-mcp-card__hint">
                      Writes `spotify-config.json` in the server folder.
                    </p>
                  </div>
                </div>
              </article>

              <article className="sarah-mcp-card">
                <header className="sarah-mcp-card__header">
                  <div>
                    <p className="sarah-mcp-card__title">Automation</p>
                    <p className="sarah-mcp-card__subtitle">
                      Keep Spotify MCP running with Sarah while you work.
                    </p>
                  </div>
                  <div className="sarah-mcp-card__badge">
                    <Sparkles className="size-4" />
                  </div>
                </header>
                <div className="sarah-mcp-card__body">
                  <div className="sarah-mcp-toggle-row">
                    <div>
                      <p className="sarah-mcp-toggle-row__title">Auto-start</p>
                      <p className="sarah-mcp-toggle-row__subtitle">
                        Launch the Spotify MCP when Sarah opens.
                      </p>
                    </div>
                    <Switch
                      checked={config.autoStart}
                      onCheckedChange={(value) =>
                        setConfig((current) => ({ ...current, autoStart: value }))
                      }
                    />
                  </div>
                </div>
              </article>

              <article className="sarah-mcp-card sarah-mcp-card--full">
                <header className="sarah-mcp-card__header">
                  <div>
                    <p className="sarah-mcp-card__title">Tool coverage</p>
                    <p className="sarah-mcp-card__subtitle">
                      Once connected, Sarah can control these Spotify tasks.
                    </p>
                  </div>
                  <div className="sarah-mcp-card__badge">
                    <Plug className="size-4" />
                  </div>
                </header>
                <div className="sarah-mcp-card__body sarah-mcp-tool-grid">
                  <div className="sarah-mcp-tool">
                    <p className="sarah-mcp-tool__title">Playback control</p>
                    <p className="sarah-mcp-tool__subtitle">
                      Play, pause, skip, volume, and queue.
                    </p>
                  </div>
                  <div className="sarah-mcp-tool">
                    <p className="sarah-mcp-tool__title">Search and discovery</p>
                    <p className="sarah-mcp-tool__subtitle">
                      Find tracks, albums, artists, and playlists.
                    </p>
                  </div>
                  <div className="sarah-mcp-tool">
                    <p className="sarah-mcp-tool__title">Library management</p>
                    <p className="sarah-mcp-tool__subtitle">
                      Create playlists and add tracks in seconds.
                    </p>
                  </div>
                </div>
              </article>
            </section>
          </section>
        </div>
      </section>
    </main>
  );
}

export default McpMarketplaceWindow;
