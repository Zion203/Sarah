"use client"

import { AnimatePresence, motion, useDragControls } from "framer-motion"
import { ListMusic, Music2, X } from "lucide-react"
import { type RefObject, useEffect, useMemo, useRef, useState } from "react"
import { listen } from "@tauri-apps/api/event"

import {
  AudioPlayerButton,
  AudioPlayerDuration,
  AudioPlayerProgress,
  AudioPlayerSpeed,
  AudioPlayerTime,
  exampleTracks,
  useAudioPlayer,
} from "@/components/ui/audio-player"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Matrix } from "@/components/ui/matrix"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

type Track = {
  id: string
  title: string
  artist: string
  vibe: string
  url: string
}

const spotifyTestTracks: Track[] = exampleTracks.slice(0, 4).map((track, index) => ({
  id: track.id,
  title: `MCP Signal ${index + 1}`,
  artist: "Spotify MCP",
  vibe: "Prototype Session",
  url: track.url,
}))

interface SpotifyAudioPlayerProps {
  isOpen: boolean
  onOpenChange: (open: boolean) => void
  onPlayingChange?: (isPlaying: boolean) => void
  draggable?: boolean
  autoplayOnOpen?: boolean
}

function useAudioLevels(
  audioRef: RefObject<HTMLAudioElement | null>,
  columns: number,
  isActive: boolean
) {
  const [levels, setLevels] = useState<number[]>(
    () => new Array(columns).fill(0)
  )
  const analyserRef = useRef<AnalyserNode | null>(null)
  const dataRef = useRef<Uint8Array<ArrayBuffer> | null>(null)
  const contextRef = useRef<AudioContext | null>(null)
  const rafRef = useRef<number | null>(null)

  useEffect(() => {
    if (!isActive || !audioRef.current || analyserRef.current) return

    const AudioContextImpl =
      window.AudioContext || (window as typeof window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext

    if (!AudioContextImpl) return

    try {
      const context = new AudioContextImpl()
      const analyser = context.createAnalyser()
      analyser.fftSize = 256
      analyser.smoothingTimeConstant = 0.82

      const source = context.createMediaElementSource(audioRef.current)
      source.connect(analyser)
      analyser.connect(context.destination)

      contextRef.current = context
      analyserRef.current = analyser
      dataRef.current = new Uint8Array(analyser.frequencyBinCount)

      if (context.state === "suspended") {
        void context.resume()
      }

      return () => {
        if (rafRef.current) {
          cancelAnimationFrame(rafRef.current)
        }
        source.disconnect()
        analyser.disconnect()
        void context.close()
      }
    } catch (error) {
      console.warn("Audio visualizer unavailable:", error)
    }
  }, [audioRef, isActive])

  useEffect(() => {
    if (!analyserRef.current || !dataRef.current) return

    if (!isActive) {
      setLevels(new Array(columns).fill(0))
      return
    }

    const analyser = analyserRef.current
    const data = dataRef.current!
    const update = () => {
      analyser.getByteFrequencyData(data)

      const step = Math.max(1, Math.floor(data.length / columns))
      const nextLevels = new Array(columns).fill(0).map((_, index) => {
        let sum = 0
        const start = index * step
        const end = Math.min(data.length, start + step)
        for (let i = start; i < end; i += 1) {
          sum += data[i]
        }
        const value = sum / Math.max(1, end - start) / 255
        return Math.min(1, Math.max(0, value))
      })

      setLevels(nextLevels)
      rafRef.current = requestAnimationFrame(update)
    }

    rafRef.current = requestAnimationFrame(update)

    return () => {
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current)
      }
    }
  }, [columns, isActive])

  return levels
}

export function SpotifyAudioPlayer({
  isOpen,
  onOpenChange,
  onPlayingChange,
  draggable = true,
  autoplayOnOpen = false,
}: SpotifyAudioPlayerProps) {
  const player = useAudioPlayer<Track>()
  const dragControls = useDragControls()
  const dockRef = useRef<HTMLElement | null>(null)
  const autoplayAttemptedRef = useRef(false)

  const trackItems = useMemo(
    () =>
      spotifyTestTracks.map((track) => ({
        id: track.id,
        src: track.url,
        data: track,
      })),
    []
  )

  const activeTrack = player.activeItem?.data ?? trackItems[0]?.data
  const isVisible = isOpen
  const levels = useAudioLevels(player.ref, 28, player.isPlaying)

  useEffect(() => {
    if (!player.activeItem && trackItems.length > 0) {
      void player.setActiveItem(trackItems[0])
    }
  }, [player.activeItem, player.setActiveItem, trackItems])

  useEffect(() => {
    let unlisten: null | (() => void) = null
    let disposed = false

    const handleCommand = async (action: string) => {
      if (!trackItems.length) return
      const currentIndex = player.activeItem
        ? trackItems.findIndex((item) => item.id === player.activeItem?.id)
        : 0
      const safeIndex = currentIndex >= 0 ? currentIndex : 0

      switch (action) {
        case "play":
          await player.play(player.activeItem ?? trackItems[0])
          onOpenChange(true)
          return
        case "pause":
          player.pause()
          return
        case "stop":
          player.pause()
          player.seek(0)
          return
        case "next": {
          const next = trackItems[(safeIndex + 1) % trackItems.length]
          await player.play(next)
          return
        }
        case "prev": {
          const prev =
            trackItems[(safeIndex - 1 + trackItems.length) % trackItems.length]
          await player.play(prev)
          return
        }
        default:
          return
      }
    }

    void listen<{ action: string }>("sarah://audio-control", (event) => {
      if (!event.payload?.action) return
      void handleCommand(event.payload.action)
    })
      .then((dispose) => {
        if (disposed) {
          dispose()
          return
        }
        unlisten = dispose
      })
      .catch(() => {
        // Ignore if not running in Tauri context.
      })

    return () => {
      disposed = true
      if (unlisten) {
        unlisten()
      }
    }
  }, [onOpenChange, player, trackItems])

  useEffect(() => {
    onPlayingChange?.(player.isPlaying)
  }, [onPlayingChange, player.isPlaying])

  useEffect(() => {
    if (!autoplayOnOpen || !isOpen) {
      autoplayAttemptedRef.current = false
      return
    }

    if (autoplayAttemptedRef.current || player.isPlaying) {
      return
    }

    const item = player.activeItem ?? trackItems[0]
    if (!item) return

    autoplayAttemptedRef.current = true
    void player.play(item).catch(() => {
      // Autoplay can be blocked without a direct user gesture.
    })
  }, [autoplayOnOpen, isOpen, player, player.activeItem, player.isPlaying, trackItems])

  const isDraggable = draggable

  return (
    <AnimatePresence initial={false}>
      {isVisible && (
        <motion.section
          className="sarah-audio-dock sarah-audio-dock--floating"
          initial={{ opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.985 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          drag={isDraggable}
          dragControls={isDraggable ? dragControls : undefined}
          dragListener={false}
          dragMomentum={false}
          dragElastic={0.12}
          aria-label="Spotify MCP audio player"
          data-tauri-disable-drag-region="true"
          ref={dockRef}
        >
          <div
            className={cn(
              "sarah-audio-dock__header",
              isDraggable && "sarah-audio-dock__drag"
            )}
            onPointerDown={(event) => {
              if (isDraggable) {
                dragControls.start(event)
              }
            }}
          >
            <div className="sarah-audio-dock__title">
              <span className="sarah-audio-dock__icon">
                <Music2 className="size-3.5" />
              </span>
              Spotify MCP
            </div>
            <div className="sarah-audio-dock__actions">
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    className="sarah-audio-control"
                    aria-label="Choose a track"
                  >
                    <ListMusic className="size-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent
                  align="end"
                  className="min-w-[200px] sarah-audio-menu"
                >
                  {trackItems.map((item) => (
                    <DropdownMenuItem
                      key={item.id}
                      onClick={() => player.play(item)}
                      className={cn(
                        "flex items-center justify-between",
                        player.activeItem?.id === item.id && "font-medium"
                      )}
                    >
                      <span>{item.data?.title ?? "Track"}</span>
                      {player.activeItem?.id === item.id ? (
                        <span className="text-xs text-muted-foreground">Playing</span>
                      ) : null}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
              <AudioPlayerSpeed
                variant="ghost"
                size="icon-sm"
                className="sarah-audio-control"
                menuClassName="sarah-audio-menu"
              />
              <Button
                variant="ghost"
                size="icon-sm"
                className="sarah-audio-control"
                onClick={() => onOpenChange(false)}
                aria-label="Hide audio player"
              >
                <X className="size-4" />
              </Button>
            </div>
          </div>

          <div className="sarah-audio-main">
            <div className="sarah-audio-cover">
              <div className="sarah-audio-cover__glow" />
              <Matrix
                rows={14}
                cols={28}
                mode="audio"
                levels={levels}
                size={3.2}
                gap={1}
                palette={{
                  on: "var(--sarah-audio-accent)",
                  off: "color-mix(in oklab, var(--sarah-audio-accent) 14%, transparent)",
                }}
                brightness={1.4}
                ariaLabel="Audio visualizer"
                className="sarah-audio-visual"
              />
            </div>

            <div className="sarah-audio-meta">
              <div className="sarah-audio-title">
                {activeTrack?.title ?? "Select a track"}
              </div>
              <div className="sarah-audio-artist">
                {activeTrack?.artist ?? "Spotify MCP"}
                {activeTrack?.vibe ? (
                  <span className="sarah-audio-chip">{activeTrack.vibe}</span>
                ) : null}
              </div>
              <div className="sarah-audio-progress-row">
                <AudioPlayerTime />
                <AudioPlayerProgress className="sarah-audio-progress" />
                <AudioPlayerDuration />
              </div>
            </div>

            <div className="sarah-audio-controls">
              <AudioPlayerButton
                size="icon"
                className="sarah-audio-play"
                item={player.activeItem ?? trackItems[0]}
              />
            </div>
          </div>
        </motion.section>
      )}
    </AnimatePresence>
  )
}
