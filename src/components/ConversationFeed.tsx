import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
} from "react";
import { AnimatePresence, animate, motion, useMotionValue } from "framer-motion";
import { AlertTriangle, ChevronLeft, ChevronRight } from "lucide-react";
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
  isScreenAccessDisabled?: boolean;
  isModelPickerVisible?: boolean;
  isWindowSourceSelection?: boolean;
  modelPickerEmptyText?: string;
  modelPickerTitle?: string;
  modelOptions?: string[];
  modelOptionsError?: null | string;
  modelOptionsLoading?: boolean;
  onModelSelect?: (model: string) => void;
  onWindowSourceSelect?: (source: DesktopWindowSource) => void;
  onSlashCommandSelect?: (command: string) => void;
  selectedModel?: string;
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
  isScreenAccessDisabled = false,
  isModelPickerVisible = false,
  isWindowSourceSelection = false,
  items,
  modelPickerEmptyText = "No local Ollama models found.",
  modelPickerTitle = "Available models",
  modelOptions = [],
  modelOptionsError = null,
  modelOptionsLoading = false,
  onModelSelect,
  onWindowSourceSelect,
  onSlashCommandSelect,
  selectedModel = "",
  showSlashCommands = false,
  slashCommandQuery = "",
  slashCommands = [],
  windowSourceError = null,
  windowSourceLoading = false,
  windowSources = [],
}: ConversationFeedProps) {
  const scrollRef = useRef<HTMLElement | null>(null);
  const windowSourceViewportRef = useRef<HTMLDivElement | null>(null);
  const windowSourceTrackRef = useRef<HTMLDivElement | null>(null);
  const trackAnimationRef = useRef<null | ReturnType<typeof animate>>(null);
  const hasDraggedWindowListRef = useRef(false);
  const suppressWindowSelectClickRef = useRef(false);
  const suppressWindowSelectTimeoutRef = useRef<null | number>(null);
  const activeWindowSliderPointerIdRef = useRef<null | number>(null);
  const pointerDownWindowTrackXRef = useRef(0);
  const pointerDownClientXRef = useRef(0);
  const lastPointerClientXRef = useRef(0);
  const lastPointerTimestampRef = useRef(0);
  const windowSliderVelocityRef = useRef(0);

  const [phraseIndex, setPhraseIndex] = useState(0);
  const [typedResponse, setTypedResponse] = useState("");
  const [windowSliderLimits, setWindowSliderLimits] = useState({ left: 0, right: 0 });
  const [isWindowListDragging, setIsWindowListDragging] = useState(false);
  const [canScrollWindowListLeft, setCanScrollWindowListLeft] = useState(false);
  const [canScrollWindowListRight, setCanScrollWindowListRight] = useState(false);

  const windowTrackX = useMotionValue(0);

  const currentItem = items.length > 0 ? items[items.length - 1] : undefined;
  const isThinking = currentItem?.status === "thinking";
  const isEmpty = !currentItem && !showSlashCommands && !isModelPickerVisible;
  const thinkingPhrase = THINKING_PHRASES[phraseIndex];
  const explicitThinkingLabel =
    currentItem?.status === "thinking" ? currentItem.response.trim() : "";
  const thinkingLabel = explicitThinkingLabel || thinkingPhrase;
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
    if (!isThinking || explicitThinkingLabel) {
      setPhraseIndex(0);
      return;
    }

    const intervalId = window.setInterval(() => {
      setPhraseIndex((current) => (current + 1) % THINKING_PHRASES.length);
    }, 1800);

    return () => window.clearInterval(intervalId);
  }, [explicitThinkingLabel, isThinking]);

  const clampWindowTrackX = useCallback(
    (value: number) => {
      return Math.min(windowSliderLimits.right, Math.max(windowSliderLimits.left, value));
    },
    [windowSliderLimits.left, windowSliderLimits.right],
  );

  const stopWindowTrackAnimation = useCallback(() => {
    if (trackAnimationRef.current) {
      trackAnimationRef.current.stop();
      trackAnimationRef.current = null;
    }
  }, []);

  const updateWindowListScrollControls = useCallback(
    (xValue: number) => {
      setCanScrollWindowListLeft(xValue < -1);
      setCanScrollWindowListRight(xValue > windowSliderLimits.left + 1);
    },
    [windowSliderLimits.left],
  );

  const animateWindowTrackTo = useCallback(
    (target: number) => {
      const next = clampWindowTrackX(target);
      stopWindowTrackAnimation();
      trackAnimationRef.current = animate(windowTrackX, next, {
        type: "spring",
        stiffness: 380,
        damping: 34,
        mass: 0.66,
      });
    },
    [clampWindowTrackX, stopWindowTrackAnimation, windowTrackX],
  );

  const updateWindowSliderGeometry = useCallback(() => {
    const viewport = windowSourceViewportRef.current;
    const track = windowSourceTrackRef.current;

    if (!viewport || !track) {
      setWindowSliderLimits({ left: 0, right: 0 });
      windowTrackX.set(0);
      updateWindowListScrollControls(0);
      return;
    }

    const leftLimit = Math.min(0, viewport.clientWidth - track.scrollWidth);
    const rightLimit = 0;

    setWindowSliderLimits((prev) => {
      if (prev.left === leftLimit && prev.right === rightLimit) {
        return prev;
      }
      return { left: leftLimit, right: rightLimit };
    });

    const clampedX = Math.min(rightLimit, Math.max(leftLimit, windowTrackX.get()));
    if (clampedX !== windowTrackX.get()) {
      windowTrackX.set(clampedX);
    }
    updateWindowListScrollControls(clampedX);
  }, [updateWindowListScrollControls, windowTrackX]);

  useEffect(() => {
    if (
      !isWindowSourceSelection ||
      windowSourceLoading ||
      windowSourceError !== null ||
      windowSources.length === 0
    ) {
      activeWindowSliderPointerIdRef.current = null;
      windowSliderVelocityRef.current = 0;
      setIsWindowListDragging(false);
      setCanScrollWindowListLeft(false);
      setCanScrollWindowListRight(false);
      setWindowSliderLimits({ left: 0, right: 0 });
      windowTrackX.set(0);
      return;
    }

    const frameId = window.requestAnimationFrame(() => updateWindowSliderGeometry());
    const onResize = () => updateWindowSliderGeometry();
    window.addEventListener("resize", onResize);

    return () => {
      window.cancelAnimationFrame(frameId);
      window.removeEventListener("resize", onResize);
    };
  }, [
    isWindowSourceSelection,
    updateWindowSliderGeometry,
    windowSourceError,
    windowSourceLoading,
    windowSources,
    windowTrackX,
  ]);

  useEffect(
    () => () => {
      stopWindowTrackAnimation();
      if (suppressWindowSelectTimeoutRef.current !== null) {
        window.clearTimeout(suppressWindowSelectTimeoutRef.current);
      }
    },
    [stopWindowTrackAnimation, suppressWindowSelectTimeoutRef],
  );

  useEffect(() => {
    const unsubscribe = windowTrackX.on("change", (xValue) => {
      updateWindowListScrollControls(xValue);
    });

    return () => unsubscribe();
  }, [updateWindowListScrollControls, windowTrackX]);

  const scrollWindowListBy = useCallback(
    (offset: number) => {
      animateWindowTrackTo(windowTrackX.get() + offset);
    },
    [animateWindowTrackTo, windowTrackX],
  );

  const finishWindowListPointerDrag = useCallback(() => {
    setIsWindowListDragging(false);
    const inertialTarget = windowTrackX.get() + windowSliderVelocityRef.current * 220;
    animateWindowTrackTo(inertialTarget);
    activeWindowSliderPointerIdRef.current = null;
    windowSliderVelocityRef.current = 0;

    if (hasDraggedWindowListRef.current) {
      suppressWindowSelectClickRef.current = true;
      if (suppressWindowSelectTimeoutRef.current !== null) {
        window.clearTimeout(suppressWindowSelectTimeoutRef.current);
      }
      suppressWindowSelectTimeoutRef.current = window.setTimeout(() => {
        suppressWindowSelectClickRef.current = false;
        suppressWindowSelectTimeoutRef.current = null;
      }, 110);
    }
  }, [animateWindowTrackTo, windowTrackX]);

  const handleWindowListPointerDownCapture = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (event.button !== 0 || windowSliderLimits.left === 0) {
        return;
      }

      stopWindowTrackAnimation();
      hasDraggedWindowListRef.current = false;
      activeWindowSliderPointerIdRef.current = event.pointerId;
      pointerDownClientXRef.current = event.clientX;
      pointerDownWindowTrackXRef.current = windowTrackX.get();
      lastPointerClientXRef.current = event.clientX;
      lastPointerTimestampRef.current = performance.now();
      windowSliderVelocityRef.current = 0;
    },
    [stopWindowTrackAnimation, windowSliderLimits.left, windowTrackX],
  );

  const handleWindowListPointerMoveCapture = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (activeWindowSliderPointerIdRef.current !== event.pointerId) {
        return;
      }

      const deltaX = event.clientX - pointerDownClientXRef.current;
      if (!hasDraggedWindowListRef.current && Math.abs(deltaX) <= 4) {
        return;
      }

      if (!hasDraggedWindowListRef.current) {
        hasDraggedWindowListRef.current = true;
        setIsWindowListDragging(true);
        event.currentTarget.setPointerCapture(event.pointerId);
      }

      const nextX = clampWindowTrackX(pointerDownWindowTrackXRef.current + deltaX);
      windowTrackX.set(nextX);

      const now = performance.now();
      const elapsed = Math.max(1, now - lastPointerTimestampRef.current);
      const deltaPointerX = event.clientX - lastPointerClientXRef.current;
      windowSliderVelocityRef.current = deltaPointerX / elapsed;
      lastPointerTimestampRef.current = now;
      lastPointerClientXRef.current = event.clientX;

      if (hasDraggedWindowListRef.current) {
        event.preventDefault();
      }
    },
    [clampWindowTrackX, windowTrackX],
  );

  const handleWindowListPointerUpCapture = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (activeWindowSliderPointerIdRef.current !== event.pointerId) {
        return;
      }

      if (
        hasDraggedWindowListRef.current &&
        event.currentTarget.hasPointerCapture(event.pointerId)
      ) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
      finishWindowListPointerDrag();
    },
    [finishWindowListPointerDrag],
  );

  const handleWindowListPointerCancelCapture = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (activeWindowSliderPointerIdRef.current !== event.pointerId) {
        return;
      }

      if (
        hasDraggedWindowListRef.current &&
        event.currentTarget.hasPointerCapture(event.pointerId)
      ) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
      finishWindowListPointerDrag();
    },
    [finishWindowListPointerDrag],
  );

  const handleWindowListLostPointerCapture = useCallback(() => {
    if (activeWindowSliderPointerIdRef.current !== null) {
      finishWindowListPointerDrag();
    }
  }, [finishWindowListPointerDrag]);

  const handleWindowListWheel = useCallback(
    (event: ReactWheelEvent<HTMLDivElement>) => {
      if (windowSliderLimits.left === 0) {
        return;
      }

      const dominantDelta =
        Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
      if (Math.abs(dominantDelta) < 0.35) {
        return;
      }

      const deltaScale = event.deltaMode === 1 ? 26 : event.deltaMode === 2 ? 120 : 1;
      const delta = dominantDelta * deltaScale;

      event.preventDefault();
      stopWindowTrackAnimation();
      windowTrackX.set(clampWindowTrackX(windowTrackX.get() - delta));
    },
    [clampWindowTrackX, stopWindowTrackAnimation, windowSliderLimits.left, windowTrackX],
  );

  const handleWindowSourceItemClick = useCallback(
    (event: ReactMouseEvent<HTMLButtonElement>, source: DesktopWindowSource) => {
      if (suppressWindowSelectClickRef.current) {
        event.preventDefault();
        return;
      }

      onWindowSourceSelect?.(source);
    },
    [onWindowSourceSelect],
  );

  return (
    <section
      ref={scrollRef}
      className={`sarah-chat-thread ${isEmpty ? "sarah-chat-thread--empty" : ""} ${
        showSlashCommands ? "sarah-chat-thread--commands" : ""
      } ${isModelPickerVisible ? "sarah-chat-thread--model-picker" : ""}`}
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
            slashCommands.map((item) => {
              const isScreenCommand =
                item.command.startsWith("/record") || item.command.startsWith("/take");
              const isDisabledForPermission = isScreenAccessDisabled && isScreenCommand;
              const showScreenPermissionWarning = isDisabledForPermission;

              return (
                <button
                  key={item.command}
                  type="button"
                  className="sarah-command-item"
                  onClick={() => onSlashCommandSelect?.(item.command)}
                  disabled={isDisabledForPermission}
                >
                  <div className="sarah-command-item__top">
                    <p className="sarah-command-item__command" aria-hidden="true">
                      <KbdGroup className="sarah-command-item__keys">
                        {item.command.split(" ").map((token, index) => (
                          <Kbd key={`${item.command}-${token}-${index}`} className="sarah-command-item__key">
                            {token}
                          </Kbd>
                        ))}
                      </KbdGroup>
                    </p>
                    {showScreenPermissionWarning ? (
                      <span className="sarah-command-item__warning">
                        <AlertTriangle className="size-3" />
                        Enable in Settings -&gt; Permissions
                      </span>
                    ) : null}
                  </div>
                  <p className="sarah-command-item__description">{item.description}</p>
                </button>
              );
            })
          )}
        </>
      ) : isModelPickerVisible ? (
        <>
          <p className="sarah-command-title">{modelPickerTitle}</p>
          {modelOptionsLoading ? (
            <p className="sarah-command-empty">Loading available models...</p>
          ) : modelOptionsError ? (
            <p className="sarah-command-empty">{modelOptionsError}</p>
          ) : modelOptions.length === 0 ? (
            <p className="sarah-command-empty">{modelPickerEmptyText}</p>
          ) : (
            <div className="sarah-model-picker-grid" role="listbox" aria-label="Quick switch models">
              {modelOptions.map((model) => {
                const isActive = model === selectedModel;
                return (
                  <button
                    key={model}
                    type="button"
                    role="option"
                    aria-selected={isActive}
                    className="sarah-model-picker-item"
                    data-active={isActive ? "true" : "false"}
                    onClick={() => onModelSelect?.(model)}
                  >
                    <span className="sarah-model-picker-item__name">{model}</span>
                    <span className="sarah-model-picker-item__status">
                      {isActive ? "Active" : "Switch"}
                    </span>
                  </button>
                );
              })}
            </div>
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
          {!isWindowSourceSelection && (
            <p className="sarah-response-status">
              {isThinking ? (
                <span className="sarah-response-status__phrase-viewport">
                  <AnimatePresence mode="wait" initial={false}>
                    <motion.span
                      key={thinkingLabel}
                      className="sarah-response-status__phrase"
                      initial={{ y: -14, opacity: 0 }}
                      animate={{ y: 0, opacity: 1 }}
                      exit={{ y: 14, opacity: 0 }}
                      transition={{ duration: 0.28, ease: [0.32, 0.72, 0, 1] }}
                    >
                      <span className="sarah-response-status__phrase-base">{thinkingLabel}</span>
                      <ShimmeringText
                        text={thinkingLabel}
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
          )}
          {currentItem?.status === "thinking" ? (
            <div className="sarah-response-skeleton" aria-label="Response is loading">
              <span className="sarah-response-skeleton__line" />
              <span className="sarah-response-skeleton__line sarah-response-skeleton__line--wide" />
              <span className="sarah-response-skeleton__line sarah-response-skeleton__line--mid" />
            </div>
          ) : isWindowSourceSelection ? (
            <>
              <div className="sarah-window-source-header">
                <p className="sarah-window-source-title">Active windows</p>
                <div className="sarah-window-source-scroll-actions">
                  <button
                    type="button"
                    className="sarah-window-source-scroll-button"
                    aria-label="Scroll windows left"
                    onClick={() => scrollWindowListBy(180)}
                    disabled={!canScrollWindowListLeft}
                  >
                    <ChevronLeft className="size-3.5" />
                  </button>
                  <button
                    type="button"
                    className="sarah-window-source-scroll-button"
                    aria-label="Scroll windows right"
                    onClick={() => scrollWindowListBy(-180)}
                    disabled={!canScrollWindowListRight}
                  >
                    <ChevronRight className="size-3.5" />
                  </button>
                </div>
              </div>
              {windowSourceLoading ? (
                <p className="sarah-window-source-state">Loading active windows...</p>
              ) : windowSourceError ? (
                <p className="sarah-window-source-state">{windowSourceError}</p>
              ) : windowSources.length === 0 ? (
                <p className="sarah-window-source-state">
                  No capturable windows found. Open a target window and retry.
                </p>
              ) : (
                <div
                  ref={windowSourceViewportRef}
                  className="sarah-window-source-slider"
                  data-dragging={isWindowListDragging ? "true" : "false"}
                  onWheel={handleWindowListWheel}
                  onPointerDownCapture={handleWindowListPointerDownCapture}
                  onPointerMoveCapture={handleWindowListPointerMoveCapture}
                  onPointerUpCapture={handleWindowListPointerUpCapture}
                  onPointerCancelCapture={handleWindowListPointerCancelCapture}
                  onLostPointerCapture={handleWindowListLostPointerCapture}
                >
                  <motion.div
                    ref={windowSourceTrackRef}
                    className="sarah-window-source-list"
                    aria-label="Active windows to capture"
                    data-dragging={isWindowListDragging ? "true" : "false"}
                    style={{ x: windowTrackX }}
                  >
                    {windowSources.map((source) => (
                      <button
                        key={`${source.id}-${source.title}`}
                        type="button"
                        className="sarah-window-source-list__item"
                        onClick={(event) => handleWindowSourceItemClick(event, source)}
                      >
                        <span className="sarah-window-source-list__title">{source.title}</span>
                        <span className="sarah-window-source-list__meta">{source.processName}</span>
                      </button>
                    ))}
                  </motion.div>
                </div>
              )}
            </>
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
