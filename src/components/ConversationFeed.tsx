import { useEffect, useRef } from "react";
import type { ConversationItem } from "@/hooks/useUIState";

interface ConversationFeedProps {
  items: ConversationItem[];
}

function ConversationFeed({ items }: ConversationFeedProps) {
  const scrollRef = useRef<HTMLElement | null>(null);
  const currentItem = items.length > 0 ? items[items.length - 1] : undefined;
  const isThinking = currentItem?.status === "thinking";

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [currentItem]);

  return (
    <section ref={scrollRef} className="sarah-chat-thread" aria-label="Current response panel">
      {!currentItem ? (
        <p className="sarah-chat-empty">
          AI response will appear here for the current prompt.
        </p>
      ) : (
        <article className="sarah-response-panel">
          <p
            className={`sarah-response-status ${isThinking ? "sarah-response-status--processing" : ""}`}
          >
            {isThinking ? (
              <>
                <span className="sarah-response-status__base">Thinking...</span>
                <span className="sarah-response-status__scan">Thinking...</span>
              </>
            ) : (
              "Response ready"
            )}
          </p>
          <p className="sarah-response-text">
            {currentItem.status === "thinking" ? "Preparing response..." : currentItem.response}
          </p>
        </article>
      )}
    </section>
  );
}

export default ConversationFeed;
