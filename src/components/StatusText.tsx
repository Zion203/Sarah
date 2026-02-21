import { AnimatePresence, motion } from "framer-motion";
import { useAssistantState, type AssistantMode } from "@/hooks/useAssistantState";
import { cn } from "@/lib/utils";

const STATUS_COPY: Record<AssistantMode, { title: string; subtitle: string }> = {
  idle: {
    title: "Idle",
    subtitle: "Press Space to start listening",
  },
  listening: {
    title: "Listening",
    subtitle: "Capturing your command",
  },
  thinking: {
    title: "Thinking",
    subtitle: "Reasoning through the request",
  },
  speaking: {
    title: "Speaking",
    subtitle: "Delivering response audio",
  },
  executing: {
    title: "Executing Task",
    subtitle: "Running local tools",
  },
};

interface StatusTextProps {
  compact?: boolean;
}

function StatusText({ compact = false }: StatusTextProps) {
  const { state } = useAssistantState();
  const copy = STATUS_COPY[state];

  return (
    <div
      className={cn(
        "flex flex-col justify-start",
        compact ? "min-h-0 items-center text-center" : "min-h-14 items-center text-center",
      )}
    >
      <AnimatePresence mode="wait">
        <motion.div
          key={state}
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -8 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
        >
          <p
            className={cn(
              "font-semibold tracking-[0.12em] text-slate-200/95 uppercase",
              compact ? "text-xs" : "text-sm",
            )}
          >
            {copy.title}
          </p>
          <p className={cn("mt-1 tracking-wide text-slate-400", compact ? "text-[11px]" : "text-xs")}>
            {copy.subtitle}
          </p>
        </motion.div>
      </AnimatePresence>
    </div>
  );
}

export default StatusText;
