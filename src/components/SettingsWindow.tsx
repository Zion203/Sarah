import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Gauge,
  KeyRound,
  Minus,
  MoonStar,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Sun,
  Volume2,
  X,
} from "lucide-react";
import { useMemo, useState, type ComponentType, type MouseEvent } from "react";
import { Button } from "@/components/ui/button";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Switch } from "@/components/ui/switch";
import type { ThemeMode } from "@/hooks/useTheme";

interface SettingsWindowProps {
  onToggleTheme: () => void;
  theme: ThemeMode;
}

type SettingsTab = "general" | "appearance" | "audio" | "privacy";
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

const SETTINGS_TABS: Array<{
  icon: ComponentType<{ className?: string }>;
  key: SettingsTab;
  label: string;
}> = [
  { icon: SlidersHorizontal, key: "general", label: "General" },
  { icon: Sparkles, key: "appearance", label: "Appearance" },
  { icon: Volume2, key: "audio", label: "Voice & Audio" },
  { icon: ShieldCheck, key: "privacy", label: "Privacy" },
];

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

            {activeTab === "privacy" && (
              <div className="sarah-settings-group">
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
