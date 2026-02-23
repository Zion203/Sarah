import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import type { ConversationItem } from "@/hooks/useUIState";
import type { DesktopWindowSource } from "@/types/screenSources";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { ShimmeringText } from "@/components/ui/shimmering-text";

interface SlashCommandItem {
  command: string;
  description: string;
}

interface ConversationFeedProps {
  items: ConversationItem[];
  isWindowSourceSelection?: boolean;
  onWindowSourceSelect?: (source: DesktopWindowSource) => void;
  showSlashCommands?: boolean;
  slashCommandQuery?: string;
  slashCommands?: SlashCommandItem[];
  windowSourceError?: null | string;
  windowSourceLoading?: boolean;
  windowSources?: DesktopWindowSource[];
}

const THINKING_PHRASES = [
  "Agent is thinking...",
  "Processing your request...",
  "Analyzing the data...",
  "Generating response...",
  "Almost there...",
  "Cross-checking context...",
  "Finalizing the answer...",
  "Polishing output...",
] as const;

function typingChunkSizeByLength(length: number) {
  if (length > 1400) {
    return 8;
  }
  if (length > 900) {
    return 6;
  }
  if (length > 500) {
    return 4;
  }
  if (length > 220) {
    return 2;
  }
  return 1;
}

function ConversationFeed({
  isWindowSourceSelection = false,
  items,
  onWindowSourceSelect,
  showSlashCommands = false,
  slashCommandQuery = "",
  slashCommands = [],
  windowSourceError = null,
  windowSourceLoading = false,
  windowSources = [],
}: ConversationFeedProps) {
  const scrollRef = useRef<HTMLElement | null>(null);
  const [phraseIndex, setPhraseIndex] = useState(0);
  const [typedResponse, setTypedResponse] = useState("");
  const currentItem = items.length > 0 ? items[items.length - 1] : undefined;
  const isThinking = currentItem?.status === "thinking";
  const isEmpty = !currentItem && !showSlashCommands;
  const thinkingPhrase = THINKING_PHRASES[phraseIndex];
  const isTypewriting =
    currentItem?.status === "completed" &&
    typedResponse.length < (currentItem?.response.length ?? 0);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [currentItem, typedResponse]);

  useEffect(() => {
    if (!currentItem || currentItem.status !== "completed") {
      setTypedResponse("");
      return;
    }

    const fullResponse = currentItem.response ?? "";
    if (!fullResponse) {
      setTypedResponse("");
      return;
    }

    let cursor = 0;
    setTypedResponse("");
    const chunkSize = typingChunkSizeByLength(fullResponse.length);
    const intervalId = window.setInterval(() => {
      cursor = Math.min(fullResponse.length, cursor + chunkSize);
      setTypedResponse(fullResponse.slice(0, cursor));

      if (cursor >= fullResponse.length) {
        window.clearInterval(intervalId);
      }
    }, 16);

    return () => window.clearInterval(intervalId);
  }, [currentItem?.id, currentItem?.status, currentItem?.response]);

  useEffect(() => {
    if (!isThinking) {
      setPhraseIndex(0);
      return;
    }

    const intervalId = window.setInterval(() => {
      setPhraseIndex((current) => (current + 1) % THINKING_PHRASES.length);
    }, 1800);

    return () => window.clearInterval(intervalId);
  }, [isThinking]);

  return (
    <section
      ref={scrollRef}
      className={`sarah-chat-thread ${isEmpty ? "sarah-chat-thread--empty" : ""} ${
        showSlashCommands ? "sarah-chat-thread--commands" : ""
      }`}
      aria-label="Current response panel"
    >
      {showSlashCommands ? (
        <>
          <p className="sarah-command-title">
            {slashCommandQuery ? `Commands matching "/${slashCommandQuery}"` : "Available commands"}
          </p>
          {slashCommands.length === 0 ? (
            <p className="sarah-command-empty">
              No commands match <code>/{slashCommandQuery}</code>.
            </p>
          ) : (
            slashCommands.map((item) => (
              <article key={item.command} className="sarah-command-item">
                <p className="sarah-command-item__command">
                  <code>{item.command}</code>
                </p>
                <p className="sarah-command-item__description">{item.description}</p>
              </article>
            ))
          )}
        </>
      ) : isEmpty ? (
        <>
          <p className="sarah-empty-description">Ask anything and Sarah will answer right here.</p>
          <p className="sarah-empty-shortcut">
            <span>Shortcut</span>
            <KbdGroup>
              <Kbd>Ctrl</Kbd>
              <span aria-hidden="true">+</span>
              <Kbd>Space</Kbd>
            </KbdGroup>
          </p>
        </>
      ) : (
        <>
          <p className="sarah-response-status">
            {isThinking ? (
              <span className="sarah-response-status__phrase-viewport">
                <AnimatePresence mode="wait" initial={false}>
                  <motion.span
                    key={thinkingPhrase}
                    className="sarah-response-status__phrase"
                    initial={{ y: -14, opacity: 0 }}
                    animate={{ y: 0, opacity: 1 }}
                    exit={{ y: 14, opacity: 0 }}
                    transition={{ duration: 0.28, ease: [0.32, 0.72, 0, 1] }}
                  >
                    <span className="sarah-response-status__phrase-base">{thinkingPhrase}</span>
                    <ShimmeringText
                      text={thinkingPhrase}
                      duration={1.05}
                      repeatDelay={0}
                      spread={1.3}
                      color="var(--muted-foreground)"
                      shimmerColor="var(--foreground)"
                      startOnView={false}
                      className="sarah-response-status__phrase-shimmer"
                    />
                  </motion.span>
                </AnimatePresence>
              </span>
            ) : (
              "Response ready"
            )}
          </p>
          {currentItem?.status === "thinking" ? (
            <div className="sarah-response-skeleton" aria-label="Response is loading">
              <span className="sarah-response-skeleton__line" />
              <span className="sarah-response-skeleton__line sarah-response-skeleton__line--wide" />
              <span className="sarah-response-skeleton__line sarah-response-skeleton__line--mid" />
            </div>
          ) : (
            <>
              <p className="sarah-response-text">
                {typedResponse}
                <span
                  className={`sarah-response-text__cursor ${isTypewriting ? "" : "sarah-response-text__cursor--hidden"}`}
                  aria-hidden="true"
                />
              </p>
              {isWindowSourceSelection && (
                <section className="sarah-window-source-panel" aria-label="Active windows to capture">
                  <p className="sarah-window-source-panel__title">Active windows</p>
                  {windowSourceLoading ? (
                    <p className="sarah-window-source-panel__state">Loading active windows...</p>
                  ) : windowSourceError ? (
                    <p className="sarah-window-source-panel__state">{windowSourceError}</p>
                  ) : windowSources.length === 0 ? (
                    <p className="sarah-window-source-panel__state">
                      No capturable windows found. Open a target window and retry.
                    </p>
                  ) : (
                    <div className="sarah-window-source-list">
                      {windowSources.map((source) => (
                        <button
                          key={`${source.id}-${source.title}`}
                          type="button"
                          className="sarah-window-source-list__item"
                          onClick={() => onWindowSourceSelect?.(source)}
                        >
                          <span className="sarah-window-source-list__title">{source.title}</span>
                          <span className="sarah-window-source-list__meta">{source.processName}</span>
                        </button>
                      ))}
                    </div>
                  )}
                </section>
              )}
            </>
          )}
        </>
      )}
    </section>
  );
}

export default ConversationFeed;
