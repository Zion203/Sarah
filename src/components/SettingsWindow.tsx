import { getCurrentWindow } from "@tauri-apps/api/window";
import { Command, Monitor, ShieldCheck, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";

const SETTINGS_DETAILS = [
  {
    icon: Command,
    label: "Shortcut",
    value: "Ctrl + Space",
    note: "Toggle overlay window",
  },
  {
    icon: Monitor,
    label: "Window Mode",
    value: "Always On Top",
    note: "Focused assistant overlay",
  },
  {
    icon: Sparkles,
    label: "Assistant",
    value: "Sarah AI Local",
    note: "UI-only interaction mode",
  },
  {
    icon: ShieldCheck,
    label: "Privacy",
    value: "Local Session",
    note: "No remote sync enabled",
  },
];

function SettingsWindow() {
  const handleClose = () => {
    void getCurrentWindow().close();
  };

  return (
    <main className="sarah-settings-window" aria-label="Sarah AI settings window">
      <section className="sarah-settings-panel">
        <header className="sarah-settings-panel__header">
          <div>
            <p className="sarah-settings-panel__eyebrow">Sarah AI</p>
            <h1 className="sarah-settings-panel__title">Settings</h1>
            <p className="sarah-settings-panel__subtitle">
              Assistant window and shortcut details.
            </p>
          </div>
          <Button type="button" variant="outline" size="sm" onClick={handleClose}>
            Close
          </Button>
        </header>

        <div className="sarah-settings-grid">
          {SETTINGS_DETAILS.map((item) => {
            const Icon = item.icon;
            return (
              <article key={item.label} className="sarah-settings-card">
                <div className="sarah-settings-card__label">
                  <Icon className="size-3.5" />
                  {item.label}
                </div>
                <p className="sarah-settings-card__value">{item.value}</p>
                <p className="sarah-settings-card__note">{item.note}</p>
              </article>
            );
          })}
        </div>
      </section>
    </main>
  );
}

export default SettingsWindow;
