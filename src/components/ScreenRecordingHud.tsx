import { AnimatePresence, motion } from "framer-motion";
import { Square } from "lucide-react";

interface ScreenRecordingHudProps {
  elapsedMs: number;
  isVisible: boolean;
  onStop: () => void;
}

function formatElapsedTime(elapsedMs: number) {
  const totalSeconds = Math.max(0, Math.floor(elapsedMs / 1000));
  const minutes = Math.floor(totalSeconds / 60)
    .toString()
    .padStart(2, "0");
  const seconds = (totalSeconds % 60).toString().padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function ScreenRecordingHud({ elapsedMs, isVisible, onStop }: ScreenRecordingHudProps) {
  return (
    <AnimatePresence>
      {isVisible && (
        <motion.div
          key="screen-recording-hud"
          className="sarah-screen-recording-hud"
          initial={{ opacity: 0, y: 14, scale: 0.96 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 14, scale: 0.96 }}
          transition={{ duration: 0.2, ease: [0.32, 0.72, 0, 1] }}
          role="status"
          aria-live="polite"
        >
          <div className="sarah-screen-recording-hud__copy">
            <span className="sarah-screen-recording-hud__dot" aria-hidden="true" />
            <span className="sarah-screen-recording-hud__label">Screen Recording</span>
            <span className="sarah-screen-recording-hud__timer">{formatElapsedTime(elapsedMs)}</span>
          </div>
          <button
            type="button"
            className="sarah-screen-recording-hud__stop"
            onClick={onStop}
            aria-label="Stop screen recording"
          >
            <Square className="size-3.5 fill-current" />
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export default ScreenRecordingHud;
