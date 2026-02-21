import { motion, useReducedMotion } from "framer-motion";
import { cn } from "@/lib/utils";
import type { UIVisualState } from "@/hooks/useUIState";

const EASE_IN_OUT: [number, number, number, number] = [0.42, 0, 0.58, 1];

interface PulseAnimation {
  animate: { scale: number | number[]; opacity: number | number[] };
  transition: { duration: number; ease?: [number, number, number, number]; repeat?: number };
}

function scaleMultiplierByState(state: UIVisualState) {
  switch (state) {
    case "idle":
      return 0.05;
    case "listening":
      return 0.11;
    case "thinking":
      return 0.15;
    case "speaking":
      return 0.16;
    default:
      return 0.05;
  }
}

function driftDurationByState(state: UIVisualState) {
  switch (state) {
    case "listening":
      return 3.8;
    case "thinking":
      return 2.8;
    case "speaking":
      return 2.2;
    case "idle":
    default:
      return 5.6;
  }
}

function pulseByState(state: UIVisualState, prefersReducedMotion: boolean): PulseAnimation {
  if (prefersReducedMotion) {
    return {
      animate: { scale: 1, opacity: 0.16 },
      transition: { duration: 0 },
    };
  }

  switch (state) {
    case "listening":
      return {
        animate: { scale: [1, 1.08, 1], opacity: [0.18, 0.3, 0.18] },
        transition: { duration: 3.2, ease: EASE_IN_OUT, repeat: Number.POSITIVE_INFINITY },
      };
    case "thinking":
      return {
        animate: { scale: [1, 1.12, 1], opacity: [0.2, 0.42, 0.2] },
        transition: { duration: 2.2, ease: EASE_IN_OUT, repeat: Number.POSITIVE_INFINITY },
      };
    case "speaking":
      return {
        animate: { scale: [1, 1.14, 1], opacity: [0.2, 0.48, 0.2] },
        transition: { duration: 1.6, ease: EASE_IN_OUT, repeat: Number.POSITIVE_INFINITY },
      };
    case "idle":
    default:
      return {
        animate: { scale: [1, 1.04, 1], opacity: [0.14, 0.2, 0.14] },
        transition: { duration: 5.4, ease: EASE_IN_OUT, repeat: Number.POSITIVE_INFINITY },
      };
  }
}

interface OrbProps {
  amplitude: number;
  state: UIVisualState;
}

function Orb({ amplitude, state }: OrbProps) {
  const prefersReducedMotion = Boolean(useReducedMotion());
  const pulseAnimation = pulseByState(state, prefersReducedMotion);
  const isGenerating = state === "thinking";
  const shellScale = prefersReducedMotion ? 1 : 1 + amplitude * scaleMultiplierByState(state);

  return (
    <motion.div
      className="sarah-mini-orb-shell"
      animate={
        isGenerating && !prefersReducedMotion
          ? { scale: shellScale, rotate: [0, 360] }
          : { scale: shellScale, rotate: 0 }
      }
      transition={
        isGenerating && !prefersReducedMotion
          ? {
              scale: { type: "spring", stiffness: 150, damping: 20, mass: 0.72 },
              rotate: {
                duration: 1.6,
                ease: "linear",
                repeat: Number.POSITIVE_INFINITY,
              },
            }
          : { type: "spring", stiffness: 150, damping: 20, mass: 0.72 }
      }
    >
      <div
        className={cn(
          "sarah-mini-orb",
          `sarah-mini-orb--${state}`,
          prefersReducedMotion && "sarah-mini-orb--still",
        )}
      >
        <motion.span
          className="sarah-mini-orb__core"
          animate={
            prefersReducedMotion
              ? { x: 0, y: 0, rotate: 0 }
              : { x: [-0.45, 0.5, -0.25], y: [0.28, -0.48, 0.2], rotate: [0, 4, -2, 0] }
          }
          transition={
            prefersReducedMotion
              ? { duration: 0 }
              : {
                  duration: driftDurationByState(state),
                  ease: "easeInOut",
                  repeat: Number.POSITIVE_INFINITY,
                }
          }
        />
        <span className="sarah-mini-orb__flow sarah-mini-orb__flow--one" />
        <span className="sarah-mini-orb__flow sarah-mini-orb__flow--two" />
        <span className="sarah-mini-orb__spectrum" />
        <motion.span
          className="sarah-mini-orb__pulse"
          animate={pulseAnimation.animate}
          transition={pulseAnimation.transition}
        />
        <span className="sarah-mini-orb__ring sarah-mini-orb__ring--outer" />
        <span className="sarah-mini-orb__ring sarah-mini-orb__ring--inner" />
      </div>
    </motion.div>
  );
}

export default Orb;
