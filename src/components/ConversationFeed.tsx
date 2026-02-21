import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import type { ConversationItem } from "@/hooks/useUIState";
import { ShimmeringText } from "@/components/ui/shimmering-text";

interface ConversationFeedProps {
  items: ConversationItem[];
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

function ConversationFeed({ items }: ConversationFeedProps) {
  const scrollRef = useRef<HTMLElement | null>(null);
  const [phraseIndex, setPhraseIndex] = useState(0);
  const [typedResponse, setTypedResponse] = useState("");
  const currentItem = items.length > 0 ? items[items.length - 1] : undefined;
  const isThinking = currentItem?.status === "thinking";
  const isEmpty = !currentItem;
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
      className={`sarah-chat-thread ${isEmpty ? "sarah-chat-thread--empty" : ""}`}
      aria-label="Current response panel"
    >
      {isEmpty ? (
        <div className="sarah-chat-empty">
          <p className="sarah-chat-empty__badge">
            <span className="sarah-chat-empty__badge-dot" />
            Ready for your prompt
          </p>
          <p className="sarah-chat-empty__title">Response panel is waiting</p>
          <p className="sarah-chat-empty__subtitle">
            Ask anything and Sarah will answer right here.
          </p>
          <div className="sarah-chat-empty__hints">
            <span className="sarah-chat-empty__hint">Try: Summarize this code</span>
            <span className="sarah-chat-empty__hint">Shortcut: Ctrl + Space</span>
          </div>
        </div>
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
          {currentItem.status === "thinking" ? (
            <div className="sarah-response-skeleton" aria-label="Response is loading">
              <span className="sarah-response-skeleton__line" />
              <span className="sarah-response-skeleton__line sarah-response-skeleton__line--wide" />
              <span className="sarah-response-skeleton__line sarah-response-skeleton__line--mid" />
            </div>
          ) : (
            <p className="sarah-response-text">
              {typedResponse}
              <span
                className={`sarah-response-text__cursor ${isTypewriting ? "" : "sarah-response-text__cursor--hidden"}`}
                aria-hidden="true"
              />
            </p>
          )}
        </>
      )}
    </section>
  );
}

export default ConversationFeed;
