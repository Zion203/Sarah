import { Orb as ElevenOrb, type AgentState } from "@/components/ui/orb";
import type { UIVisualState } from "@/hooks/useUIState";

function mapStateToAgentState(state: UIVisualState): AgentState {
  switch (state) {
    case "listening":
      return "listening";
    case "thinking":
      return "thinking";
    case "speaking":
      return "talking";
    case "idle":
    default:
      return null;
  }
}

interface OrbProps {
  amplitude: number;
  colors?: [string, string];
  state: UIVisualState;
}

function Orb({ amplitude, colors, state }: OrbProps) {
  const clampedAmplitude = Math.max(0, Math.min(1, amplitude));
  const agentState = mapStateToAgentState(state);
  const manualInput = clampedAmplitude;
  const manualOutput =
    state === "speaking"
      ? 0.88 + clampedAmplitude * 0.12
      : state === "thinking"
        ? 0.82 + clampedAmplitude * 0.18
        : state === "listening"
          ? 0.55 + clampedAmplitude * 0.2
          : 0.35;

  return (
    <div className="sarah-mini-orb-shell">
      <ElevenOrb
        className="h-full w-full"
        agentState={agentState}
        colors={colors}
        volumeMode="manual"
        manualInput={manualInput}
        manualOutput={manualOutput}
      />
    </div>
  );
}

export default Orb;
