import { useMemo } from "react";
import { MoonStar, Settings2, Square, Sun } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import Orb from "@/components/Orb";
import type { UIVisualState } from "@/hooks/useUIState";

const PLACEHOLDER_BY_STATE: Record<UIVisualState, string> = {
  idle: "Ask Sarah anything...",
  listening: "Listening...",
  thinking: "Thinking...",
  speaking: "Speaking response...",
};

interface AssistantInputProps {
  amplitude: number;
  isDarkTheme: boolean;
  onClear: () => void;
  onOpenSettings: () => void;
  onPromptChange: (value: string) => void;
  onStop: () => void;
  onSubmit: () => void;
  onToggleTheme: () => void;
  prompt: string;
  readOnly: boolean;
  showStopAction: boolean;
  state: UIVisualState;
}

function AssistantInput({
  amplitude,
  isDarkTheme,
  onClear,
  onOpenSettings,
  onPromptChange,
  onStop,
  onSubmit,
  onToggleTheme,
  prompt,
  readOnly,
  showStopAction,
  state,
}: AssistantInputProps) {
  const placeholder = useMemo(() => PLACEHOLDER_BY_STATE[state], [state]);

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    onSubmit();
  };

  return (
    <form className="sarah-input-shell" data-tauri-drag-region onSubmit={handleSubmit}>
      <div className="sarah-input-orb-wrap" data-tauri-drag-region aria-hidden="true">
        <Orb amplitude={amplitude} state={state} />
      </div>

      <Input
        data-tauri-disable-drag-region="true"
        value={prompt}
        onChange={(event) => onPromptChange(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onClear();
          }
        }}
        readOnly={readOnly}
        placeholder={placeholder}
        className="sarah-input"
        autoFocus
        autoComplete="off"
        spellCheck={false}
      />

      <div className="sarah-action-wrap" data-tauri-disable-drag-region="true">
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          onClick={onToggleTheme}
          className="sarah-action-button sarah-action-button--theme"
          aria-label={isDarkTheme ? "Switch to light theme" : "Switch to dark theme"}
          title={isDarkTheme ? "Switch to light theme" : "Switch to dark theme"}
        >
          {isDarkTheme ? <Sun className="size-3.5" /> : <MoonStar className="size-3.5" />}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          onClick={showStopAction ? onStop : onOpenSettings}
          className={`sarah-action-button ${showStopAction ? "sarah-action-button--stop" : "sarah-action-button--settings"}`}
          aria-label={showStopAction ? "Stop response" : "Open settings"}
          title={showStopAction ? "Stop response" : "Open settings"}
        >
          {showStopAction ? (
            <Square className="size-3.5 fill-current" />
          ) : (
            <Settings2 className="size-3.5" />
          )}
        </Button>
      </div>
    </form>
  );
}

export default AssistantInput;
