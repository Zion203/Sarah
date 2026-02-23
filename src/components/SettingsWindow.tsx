import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  AppWindow,
  FolderOpen,
  Gauge,
  KeyRound,
  Monitor,
  Minus,
  MoonStar,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Sun,
  Volume2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState, type ComponentType, type MouseEvent } from "react";
import { Button } from "@/components/ui/button";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Switch } from "@/components/ui/switch";
import { useAppPreferences, type ScreenCaptureSurface } from "@/hooks/useAppPreferences";
import type { ThemeMode } from "@/hooks/useTheme";
import type { DesktopWindowSource } from "@/types/screenSources";

interface SettingsWindowProps {
  onToggleTheme: () => void;
  theme: ThemeMode;
}

type SettingsTab = "general" | "appearance" | "audio" | "permissions";
const DRAG_BLOCK_SELECTOR = [
  "button",
  "input",
  "textarea",
  "select",
  "option",
  "a",
  "label",
  "[role='button']",
  ".sarah-settings-content",
  ".sarah-settings-group",
  "[data-tauri-disable-drag-region='true']",
  "[contenteditable='true']",
].join(",");

const SETTINGS_TABS: Array<{
  icon: ComponentType<{ className?: string }>;
  key: SettingsTab;
  label: string;
}> = [
  { icon: SlidersHorizontal, key: "general", label: "General" },
  { icon: Sparkles, key: "appearance", label: "Appearance" },
  { icon: Volume2, key: "audio", label: "Voice & Audio" },
  { icon: ShieldCheck, key: "permissions", label: "Permissions" },
];

function surfaceLabel(surface: ScreenCaptureSurface) {
  return surface === "window" ? "Window" : "Entire Screen";
}

function formatPermissionTimestamp(value: null | string) {
  if (!value) {
    return "Not granted yet";
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.valueOf())) {
    return "Granted";
  }

  return `Granted ${parsed.toLocaleString()}`;
}

function SettingsWindow({ onToggleTheme, theme }: SettingsWindowProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [startOnLaunch, setStartOnLaunch] = useState(true);
  const [alwaysOnTop, setAlwaysOnTop] = useState(true);
  const [responseStyle, setResponseStyle] = useState("balanced");
  const [visualEffects, setVisualEffects] = useState(true);
  const [voiceFeedback, setVoiceFeedback] = useState(true);
  const [listeningSensitivity, setListeningSensitivity] = useState("medium");
  const [localHistory, setLocalHistory] = useState(true);
  const [allowCloudSync, setAllowCloudSync] = useState(false);
  const { preferences, updatePreferences } = useAppPreferences();
  const [isPermissionCheckRunning, setIsPermissionCheckRunning] = useState(false);
  const [isSelectingCaptureDirectory, setIsSelectingCaptureDirectory] = useState(false);
  const [permissionCheckNotice, setPermissionCheckNotice] = useState<null | string>(null);
  const [defaultCaptureDirectory, setDefaultCaptureDirectory] = useState("");
  const [capturePathNotice, setCapturePathNotice] = useState<null | string>(null);
  const handleClose = async () => {
    try {
      await getCurrentWindow().close();
    } catch (error) {
      console.error("Failed to close settings window.", error);
    }
  };
  const handleMinimize = async () => {
    try {
      await getCurrentWindow().minimize();
    } catch (error) {
      console.error("Failed to minimize settings window.", error);
    }
  };
  const isDarkTheme = theme === "dark";
  const activeLabel = useMemo(
    () => SETTINGS_TABS.find((tab) => tab.key === activeTab)?.label ?? "General",
    [activeTab],
  );
  const selectedSurface = preferences.screenCaptureSurface;
  const selectedSurfacePermission = preferences.screenPermissions[selectedSurface];
  const selectedSurfacePermissionTimestamp =
    preferences.screenPermissionGrantedAt[selectedSurface];

  useEffect(() => {
    void (async () => {
      try {
        const path = await invoke<string>("get_default_capture_directory");
        setDefaultCaptureDirectory(path);
      } catch {
        setDefaultCaptureDirectory("System default");
      }
    })();
  }, []);

  const selectCapturePath = async () => {
    setIsSelectingCaptureDirectory(true);
    setCapturePathNotice(null);

    try {
      const selectedPath = await invoke<null | string>("pick_capture_output_directory", {
        initialDirectory: preferences.captureOutputDirectory ?? defaultCaptureDirectory,
      });

      if (typeof selectedPath === "string" && selectedPath.trim().length > 0) {
        updatePreferences((current) => ({
          ...current,
          captureOutputDirectory: selectedPath.trim(),
        }));
        setCapturePathNotice("Custom save location updated.");
      } else {
        setCapturePathNotice("Folder selection cancelled.");
      }
    } catch {
      setCapturePathNotice("Could not open folder picker.");
    } finally {
      setIsSelectingCaptureDirectory(false);
    }
  };

  const resetCapturePathToDefault = () => {
    updatePreferences((current) => ({
      ...current,
      captureOutputDirectory: null,
    }));
    setCapturePathNotice("Save location reset to default.");
  };

  const verifyScreenPermission = async (surface: ScreenCaptureSurface) => {
    setIsPermissionCheckRunning(true);
    setPermissionCheckNotice(null);

    try {
      let windowHwnd: string | undefined = undefined;
      if (surface === "window") {
        const windows = await invoke<DesktopWindowSource[]>("list_active_windows");
        const firstWindow = Array.isArray(windows) ? windows[0] : undefined;

        if (!firstWindow?.id) {
          setPermissionCheckNotice("No active window is available for verification.");
          updatePreferences((current) => ({
            ...current,
            screenPermissions: {
              ...current.screenPermissions,
              [surface]: false,
            },
          }));
          return;
        }

        windowHwnd = firstWindow.id;
      }

      await invoke("start_native_screen_recording", {
        surface,
        windowHwnd,
      });
      await new Promise((resolve) => window.setTimeout(resolve, 420));
      await invoke("stop_native_screen_recording");

      const grantedAt = new Date().toISOString();
      updatePreferences((current) => ({
        ...current,
        allowScreenRecording: true,
        screenPermissions: {
          ...current.screenPermissions,
          [surface]: true,
        },
        screenPermissionGrantedAt: {
          ...current.screenPermissionGrantedAt,
          [surface]: grantedAt,
        },
      }));
      setPermissionCheckNotice(`${surfaceLabel(surface)} permission granted.`);
    } catch (error) {
      const message = `${surfaceLabel(surface)} permission check failed.`;

      updatePreferences((current) => ({
        ...current,
        screenPermissions: {
          ...current.screenPermissions,
          [surface]: false,
        },
      }));
      setPermissionCheckNotice(message);
    } finally {
      setIsPermissionCheckRunning(false);
    }
  };

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
      console.error("Failed to start dragging settings window.", error);
    }
  };

  return (
    <main
      className="sarah-settings-window"
      aria-label="Sarah AI settings window"
      onMouseDownCapture={handleWindowMouseDownCapture}
    >
      <section className="sarah-settings-macos">
        <header
          className="sarah-settings-titlebar"
          onDoubleClickCapture={(event) => {
            event.preventDefault();
            event.stopPropagation();
          }}
        >
          <div className="sarah-settings-titlebar__meta">{activeLabel}</div>
          <p className="sarah-settings-titlebar__title">Sarah AI Settings</p>
          <div
            className="sarah-settings-titlebar__window-controls"
            data-tauri-disable-drag-region="true"
          >
            <button
              type="button"
              className="sarah-settings-titlebar__window-btn"
              aria-label="Minimize settings"
              data-tauri-disable-drag-region="true"
              onClick={() => void handleMinimize()}
            >
              <Minus className="size-3.5" />
            </button>
            <button
              type="button"
              className="sarah-settings-titlebar__window-btn sarah-settings-titlebar__window-btn--close"
              aria-label="Close settings"
              data-tauri-disable-drag-region="true"
              onClick={() => void handleClose()}
            >
              <X className="size-3.5" />
            </button>
          </div>
        </header>

        <div className="sarah-settings-layout">
          <aside className="sarah-settings-sidebar">
            <p className="sarah-settings-sidebar__eyebrow">Preferences</p>
            <nav className="sarah-settings-sidebar__nav" aria-label="Settings sections">
              {SETTINGS_TABS.map((tab) => {
                const Icon = tab.icon;
                return (
                  <button
                    key={tab.key}
                    type="button"
                    className={`sarah-settings-sidebar__item ${activeTab === tab.key ? "is-active" : ""}`}
                    onClick={() => setActiveTab(tab.key)}
                  >
                    <Icon className="size-3.5" />
                    <span>{tab.label}</span>
                  </button>
                );
              })}
            </nav>
            <div className="sarah-settings-sidebar__footnote">
              <KeyRound className="size-3.5" />
              <span>`Ctrl + Space` toggles the assistant</span>
            </div>
          </aside>

          <section className="sarah-settings-content">
            <header className="sarah-settings-content__hero">
              <p className="sarah-settings-content__eyebrow">OpenAI-style controls</p>
              <h1 className="sarah-settings-content__title">{activeLabel}</h1>
              <p className="sarah-settings-content__subtitle">
                Manage how Sarah appears, listens, and responds in your local session.
              </p>
            </header>
            <div
              className={`sarah-settings-content__details ${activeTab === "permissions" ? "sarah-settings-content__details--permissions" : ""}`}
            >
              {activeTab === "general" && (
                <div className="sarah-settings-group">
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Startup behavior</p>
                      <p className="sarah-settings-row__note">
                        Keep the assistant ready in the tray at launch.
                      </p>
                    </div>
                    <Switch checked={startOnLaunch} onCheckedChange={setStartOnLaunch} />
                  </article>
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Always on top</p>
                      <p className="sarah-settings-row__note">
                        Keep Sarah above other windows while you work.
                      </p>
                    </div>
                    <Switch checked={alwaysOnTop} onCheckedChange={setAlwaysOnTop} />
                  </article>
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Response style</p>
                      <p className="sarah-settings-row__note">
                        Choose how detailed generated answers should be.
                      </p>
                    </div>
                    <RadioGroup
                      value={responseStyle}
                      onValueChange={setResponseStyle}
                      className="sarah-settings-radio-group"
                    >
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="concise" />
                        <span>Concise</span>
                      </label>
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="balanced" />
                        <span>Balanced</span>
                      </label>
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="detailed" />
                        <span>Detailed</span>
                      </label>
                    </RadioGroup>
                  </article>
                </div>
              )}

              {activeTab === "appearance" && (
                <div className="sarah-settings-group">
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Theme mode</p>
                      <p className="sarah-settings-row__note">
                        Pick the mode that fits your room and screen brightness.
                      </p>
                    </div>
                    <RadioGroup
                      value={isDarkTheme ? "dark" : "light"}
                      onValueChange={(value) => {
                        if ((value === "dark") !== isDarkTheme) {
                          onToggleTheme();
                        }
                      }}
                      className="sarah-settings-radio-group"
                    >
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="light" />
                        <Sun className="size-3.5" />
                        <span>Light</span>
                      </label>
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="dark" />
                        <MoonStar className="size-3.5" />
                        <span>Dark</span>
                      </label>
                    </RadioGroup>
                  </article>
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Visual effects</p>
                      <p className="sarah-settings-row__note">
                        Keep subtle motion and active shimmer effects enabled.
                      </p>
                    </div>
                    <Switch checked={visualEffects} onCheckedChange={setVisualEffects} />
                  </article>
                </div>
              )}

              {activeTab === "audio" && (
                <div className="sarah-settings-group">
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Voice feedback</p>
                      <p className="sarah-settings-row__note">
                        Spoken response is enabled when speaking state is active.
                      </p>
                    </div>
                    <Switch checked={voiceFeedback} onCheckedChange={setVoiceFeedback} />
                  </article>
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Listening sensitivity</p>
                      <p className="sarah-settings-row__note">
                        Input detection threshold for ambient environments.
                      </p>
                    </div>
                    <RadioGroup
                      value={listeningSensitivity}
                      onValueChange={setListeningSensitivity}
                      className="sarah-settings-radio-group"
                    >
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="low" />
                        <span>Low</span>
                      </label>
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="medium" />
                        <span>Medium</span>
                      </label>
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="high" />
                        <span>High</span>
                      </label>
                    </RadioGroup>
                  </article>
                </div>
              )}

              {activeTab === "permissions" && (
                <div className="sarah-settings-group sarah-settings-group--permissions">
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Allow Read My Screen</p>
                      <p className="sarah-settings-row__note">
                        Master permission switch for <code>/record</code> and <code>/take</code>.
                      </p>
                    </div>
                    <Switch
                      checked={preferences.allowScreenRecording}
                      onCheckedChange={(checked) =>
                        updatePreferences((current) => ({
                          ...current,
                          allowScreenRecording: checked,
                        }))
                      }
                    />
                  </article>
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Default capture source</p>
                      <p className="sarah-settings-row__note">
                        <code>/record</code> and <code>/take</code> use this saved source mode.
                      </p>
                    </div>
                    <RadioGroup
                      value={selectedSurface}
                      onValueChange={(value) => {
                        const next = value === "screen" ? "screen" : "window";
                        updatePreferences((current) => ({
                          ...current,
                          screenCaptureSurface: next,
                        }));
                      }}
                      className="sarah-settings-radio-group"
                    >
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="window" />
                        <AppWindow className="size-3.5" />
                        <span>Window</span>
                      </label>
                      <label className="sarah-settings-choice">
                        <RadioGroupItem value="screen" />
                        <Monitor className="size-3.5" />
                        <span>Entire Screen</span>
                      </label>
                    </RadioGroup>
                  </article>
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Capture save location</p>
                      <p className="sarah-settings-row__note">
                        Default: <code>{defaultCaptureDirectory || "Loading..."}</code>
                      </p>
                      <p className="sarah-settings-row__note">
                        Use folder picker to set custom location for videos and screenshots.
                      </p>
                      <p className="sarah-settings-row__note">
                        Current:{" "}
                        <code>
                          {(preferences.captureOutputDirectory ?? defaultCaptureDirectory) ||
                            "Loading..."}
                        </code>
                      </p>
                      {capturePathNotice ? (
                        <p className="sarah-settings-row__note">{capturePathNotice}</p>
                      ) : null}
                    </div>
                    <div className="sarah-settings-save-location">
                      <div className="sarah-settings-save-location__actions">
                        <Button
                          type="button"
                          size="sm"
                          variant="outline"
                          disabled={isSelectingCaptureDirectory}
                          onClick={() => void selectCapturePath()}
                        >
                          <FolderOpen className="size-3.5" />
                          {isSelectingCaptureDirectory ? "Opening..." : "Choose Folder"}
                        </Button>
                        <Button
                          type="button"
                          size="sm"
                          variant="ghost"
                          onClick={resetCapturePathToDefault}
                        >
                          Use Default
                        </Button>
                      </div>
                    </div>
                  </article>
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Permission status</p>
                      <p className="sarah-settings-row__note">
                        Active mode: <strong>{surfaceLabel(selectedSurface)}</strong> •{" "}
                        {selectedSurfacePermission
                          ? formatPermissionTimestamp(selectedSurfacePermissionTimestamp)
                          : "Not granted yet"}
                      </p>
                      {permissionCheckNotice ? (
                        <p className="sarah-settings-row__note">{permissionCheckNotice}</p>
                      ) : null}
                    </div>
                    <div className="sarah-settings-permission-actions">
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        disabled={isPermissionCheckRunning}
                        onClick={() => void verifyScreenPermission(selectedSurface)}
                      >
                        {isPermissionCheckRunning ? "Checking..." : "Grant / Verify"}
                      </Button>
                      <span
                        className={`sarah-settings-permission-badge ${selectedSurfacePermission ? "is-granted" : "is-missing"}`}
                      >
                        {selectedSurfacePermission ? "Granted" : "Missing"}
                      </span>
                    </div>
                  </article>
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Local history</p>
                      <p className="sarah-settings-row__note">
                        Conversations are stored only on this device.
                      </p>
                    </div>
                    <Switch checked={localHistory} onCheckedChange={setLocalHistory} />
                  </article>
                  <article className="sarah-settings-row">
                    <div className="sarah-settings-row__copy">
                      <p className="sarah-settings-row__title">Cloud sync</p>
                      <p className="sarah-settings-row__note">
                        Remote sync is currently disabled by design.
                      </p>
                    </div>
                    <Switch checked={allowCloudSync} onCheckedChange={setAllowCloudSync} />
                  </article>
                </div>
              )}
            </div>

            <footer className="sarah-settings-footer">
              <div className="sarah-settings-footer__chip">
                <Gauge className="size-3.5" />
                Stable local runtime
              </div>
              <Button type="button" variant="outline" size="sm" onClick={handleClose}>
                Done
              </Button>
            </footer>
          </section>
        </div>
      </section>
    </main>
  );
}

export default SettingsWindow;
