"use client"

import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { AnimatePresence, motion, useDragControls } from "framer-motion"
import {
  Gauge,
  Loader2,
  Music2,
  Pause,
  Play,
  SkipBack,
  SkipForward,
  X,
} from "lucide-react"
import { useCallback, useEffect, useRef, useState } from "react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Matrix } from "@/components/ui/matrix"
import { cn } from "@/lib/utils"

const MCP_STORAGE_KEY = "sarah_spotify_mcp_config_v1"
const SPOTIFY_WEB_DEVICE_STORAGE_KEY = "sarah_spotify_web_sdk_device_id_v1"
const MCP_LEGACY_ROOT =
  "C:\\Users\\jesud\\OneDrive\\Desktop\\personal\\Sarah\\mcp\\spotify-mcp-server"
const MCP_DEFAULT_ROOT = "F:\\Sarah\\mcp\\spotify-mcp-server"

interface SpotifyAudioPlayerProps {
  isOpen: boolean
  onOpenChange: (open: boolean) => void
  onPlayingChange?: (isPlaying: boolean) => void
  draggable?: boolean
  autoplayOnOpen?: boolean
  windowTitlebarDrag?: boolean
}

interface SpotifyPlaybackState {
  artist: string
  deviceLabel: string
  durationSec: number
  hasTrack: boolean
  isPlaying: boolean
  progressSec: number
  title: string
  volumeLabel: string
}

interface SpotifyToolResponse {
  content?: Array<{ isError?: boolean; text?: string; type?: string }>
}

interface SpotifyConfigSnapshot {
  accessToken?: string | null
  expiresAt?: number | null
}

interface SpotifyWebPlaybackPlayer {
  addListener: (event: string, callback: (payload: unknown) => void) => boolean
  connect: () => Promise<boolean>
  disconnect: () => void
  getCurrentState?: () => Promise<SpotifyWebPlaybackState | null>
  seek?: (positionMs: number) => Promise<void>
}

interface SpotifyWebPlaybackState {
  paused?: boolean
  position?: number
  track_window?: {
    current_track?: {
      artists?: Array<{ name?: string }>
      duration_ms?: number
      name?: string
    }
  }
}

declare global {
  interface Window {
    Spotify?: {
      Player: new (options: {
        name: string
        getOAuthToken: (callback: (token: string) => void) => void
        volume?: number
      }) => SpotifyWebPlaybackPlayer
    }
    onSpotifyWebPlaybackSDKReady?: () => void
  }
}

let spotifySdkLoadPromise: Promise<void> | null = null

function ensureSpotifyWebSdkLoaded() {
  if (typeof window === "undefined") {
    return Promise.reject(new Error("Sarah Audio is only available in the browser."))
  }

  if (window.Spotify?.Player) {
    return Promise.resolve()
  }

  if (spotifySdkLoadPromise) {
    return spotifySdkLoadPromise
  }

  spotifySdkLoadPromise = new Promise<void>((resolve, reject) => {
    const existingScript = document.querySelector<HTMLScriptElement>(
      'script[data-spotify-web-sdk="true"]',
    )

    const finalizeReady = () => {
      if (window.Spotify?.Player) {
        resolve()
        return
      }

      reject(new Error("Sarah Audio loaded without Spotify player object."))
    }

    const previousReady = window.onSpotifyWebPlaybackSDKReady
    window.onSpotifyWebPlaybackSDKReady = () => {
      previousReady?.()
      finalizeReady()
    }

    if (existingScript) {
      existingScript.addEventListener("error", () => {
        reject(new Error("Failed to load Spotify Web Playback SDK script."))
      })
      return
    }

    const script = document.createElement("script")
    script.src = "https://sdk.scdn.co/spotify-player.js"
    script.async = true
    script.dataset.spotifyWebSdk = "true"
    script.onerror = () => {
      reject(new Error("Failed to load Spotify Web Playback SDK script."))
    }
    document.head.appendChild(script)
  })

  return spotifySdkLoadPromise
}

function readMcpServerRoot() {
  if (typeof window === "undefined") {
    return MCP_DEFAULT_ROOT
  }

  const raw = window.localStorage.getItem(MCP_STORAGE_KEY)
  if (!raw) {
    return MCP_DEFAULT_ROOT
  }

  try {
    const parsed = JSON.parse(raw) as { serverRoot?: unknown }
    if (typeof parsed.serverRoot !== "string") {
      return MCP_DEFAULT_ROOT
    }

    const normalized = parsed.serverRoot.trim()
    if (!normalized || normalized === MCP_LEGACY_ROOT) {
      return MCP_DEFAULT_ROOT
    }

    return normalized
  } catch {
    return MCP_DEFAULT_ROOT
  }
}

function formatClock(totalSeconds: number) {
  const safe = Number.isFinite(totalSeconds) ? Math.max(0, Math.floor(totalSeconds)) : 0
  const hours = Math.floor(safe / 3600)
  const minutes = Math.floor((safe % 3600) / 60)
  const seconds = safe % 60

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`
  }

  return `${minutes}:${seconds.toString().padStart(2, "0")}`
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

function parseToolOutput(raw: string) {
  const trimmed = raw.trim()
  if (!trimmed) {
    return { isError: true, text: "Spotify returned empty output." }
  }

  try {
    const parsed = JSON.parse(trimmed) as SpotifyToolResponse
    const blocks = Array.isArray(parsed.content) ? parsed.content : []
    const text = blocks
      .map((block) => (typeof block.text === "string" ? block.text.trim() : ""))
      .filter(Boolean)
      .join("\n\n")
      .trim()
    const inferredError =
      /^error\b/i.test(text) ||
      /\b(failed|not found|missing|unauthorized|forbidden)\b/i.test(text)

    return {
      isError: blocks.some((block) => block.isError === true) || inferredError,
      text: text || trimmed,
    }
  } catch {
    return { isError: false, text: trimmed }
  }
}

function toErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string" &&
    (error as { message: string }).message.trim()
  ) {
    return (error as { message: string }).message
  }

  if (typeof error === "string" && error.trim()) {
    return error
  }

  return fallback
}

function hashTrackSeed(value: string) {
  let hash = 0
  for (let index = 0; index < value.length; index++) {
    hash = (hash * 31 + value.charCodeAt(index)) | 0
  }
  return Math.abs(hash)
}

function useAudioLevels(columns: number, playback: Pick<
  SpotifyPlaybackState,
  "artist" | "durationSec" | "hasTrack" | "isPlaying" | "progressSec" | "title"
>) {
  const [levels, setLevels] = useState<number[]>(() => new Array(columns).fill(0))
  const trackSeed = hashTrackSeed(`${playback.title}|${playback.artist}`)
  const isActive = playback.isPlaying && playback.hasTrack

  useEffect(() => {
    let raf = 0
    let lastPaint = 0
    const fpsInterval = 1000 / 30

    const tick = (now: number) => {
      if (now - lastPaint < fpsInterval) {
        raf = window.requestAnimationFrame(tick)
        return
      }
      lastPaint = now

      const playheadRatio =
        playback.durationSec > 0
          ? clamp(playback.progressSec / playback.durationSec, 0, 1)
          : 0
      const phase = now * 0.005 + playheadRatio * Math.PI * 18 + (trackSeed % 1000) * 0.0007
      const floor = isActive ? 0.08 : 0.03
      const amplitude = isActive ? 0.82 : 0.14

      setLevels(
        Array.from({ length: columns }, (_, index) => {
          const colRatio = index / Math.max(1, columns - 1)
          const harmonicA = Math.sin(phase + colRatio * Math.PI * 2.8)
          const harmonicB = Math.sin(phase * 1.9 - colRatio * Math.PI * 5.3 + (trackSeed % 83))
          const harmonicC = Math.sin(phase * 0.75 + colRatio * Math.PI * 11.4)
          const blend = Math.abs(harmonicA * 0.56 + harmonicB * 0.29 + harmonicC * 0.15)
          const contour = 0.78 + 0.22 * Math.sin(phase * 0.35 + colRatio * Math.PI * 4)
          return clamp(floor + amplitude * blend * contour, 0, 1)
        }),
      )

      raf = window.requestAnimationFrame(tick)
    }

    raf = window.requestAnimationFrame(tick)
    return () => window.cancelAnimationFrame(raf)
  }, [
    columns,
    isActive,
    playback.durationSec,
    playback.progressSec,
    trackSeed,
  ])

  return levels
}

function SpotifyAudioPlayer({
  isOpen,
  onOpenChange,
  onPlayingChange,
  draggable = true,
  autoplayOnOpen = false,
  windowTitlebarDrag = false,
}: SpotifyAudioPlayerProps) {
  const dragControls = useDragControls()
  const dockRef = useRef<HTMLElement | null>(null)
  const progressTrackRef = useRef<HTMLDivElement | null>(null)
  const autoplayAttemptedRef = useRef(false)
  const sdkPlayerRef = useRef<SpotifyWebPlaybackPlayer | null>(null)
  const sdkDeviceIdRef = useRef("")
  const [isActionPending, setIsActionPending] = useState(false)
  const [statusText, setStatusText] = useState("Sarah Audio is initializing.")
  const [playback, setPlayback] = useState<SpotifyPlaybackState>({
    artist: "Spotify",
    deviceLabel: "No active device",
    durationSec: 0,
    hasTrack: false,
    isPlaying: false,
    progressSec: 0,
    title: "Nothing is currently playing",
    volumeLabel: "--",
  })

  const isVisible = isOpen
  const levels = useAudioLevels(28, playback)

  const runSpotifyTool = useCallback(
    async (tool: string, args: Record<string, unknown>) => {
      const serverRoot = readMcpServerRoot()
      const invokeTool = async (nextArgs: Record<string, unknown>) =>
        invoke<string>("run_spotify_tool", {
          args: nextArgs,
          serverRoot,
          tool,
        })

      let parsed = parseToolOutput(await invokeTool(args))
      const deviceId = typeof args.deviceId === "string" ? args.deviceId.trim() : ""
      const shouldRetryWithoutDevice =
        Boolean(deviceId) && parsed.isError && /\bdevice not found\b/i.test(parsed.text)

      if (shouldRetryWithoutDevice) {
        if (typeof window !== "undefined") {
          window.localStorage.removeItem(SPOTIFY_WEB_DEVICE_STORAGE_KEY)
        }
        if (deviceId && sdkDeviceIdRef.current === deviceId) {
          sdkDeviceIdRef.current = ""
        }

        const retryArgs: Record<string, unknown> = { ...args }
        delete retryArgs.deviceId
        parsed = parseToolOutput(await invokeTool(retryArgs))
      }

      return parsed
    },
    [],
  )

  const readSpotifySnapshot = useCallback(async () => {
    const serverRoot = readMcpServerRoot()
    return invoke<SpotifyConfigSnapshot>("read_spotify_config", { serverRoot })
  }, [])

  const getSpotifySdkToken = useCallback(async () => {
    const initialSnapshot = await readSpotifySnapshot()
    const initialToken = initialSnapshot.accessToken?.trim() || ""
    const expiresAt = initialSnapshot.expiresAt ?? null
    const isFreshToken =
      initialToken.length > 0 &&
      (typeof expiresAt !== "number" || expiresAt > Date.now() + 45_000)

    if (isFreshToken) {
      return initialToken
    }

    const refreshAttempt = await runSpotifyTool("getAvailableDevices", {})
    if (
      refreshAttempt.isError &&
      /\b(oauth|re-auth|reauth|refresh token|authorization|authenticate)\b/i.test(
        refreshAttempt.text,
      )
    ) {
      throw new Error(refreshAttempt.text)
    }

    const refreshedSnapshot = await readSpotifySnapshot()
    const refreshedToken = refreshedSnapshot.accessToken?.trim() || ""
    if (!refreshedToken) {
      throw new Error(
        "Spotify token is missing. Run OAuth again from MCP Marketplace and include streaming scope.",
      )
    }

    return refreshedToken
  }, [readSpotifySnapshot, runSpotifyTool])

  const applyWebPlaybackState = useCallback((state: SpotifyWebPlaybackState | null) => {
    const track = state?.track_window?.current_track
    if (!track || !track.name?.trim()) {
      setPlayback((current) => ({
        ...current,
        hasTrack: false,
        isPlaying: false,
        progressSec: 0,
      }))
      setStatusText(
        sdkDeviceIdRef.current
          ? "Nothing is playing on Sarah device yet. Ask Sarah to play a song."
          : "Sarah Audio device is not ready yet.",
      )
      return
    }

    const title = track.name.trim()
    const artist = Array.isArray(track.artists)
      ? track.artists
          .map((entry) => entry?.name?.trim() || "")
          .filter(Boolean)
          .join(", ")
      : "Spotify"
    const durationSec = Math.max(
      0,
      Math.floor(Number.isFinite(track.duration_ms) ? (track.duration_ms as number) / 1000 : 0),
    )
    const progressSec = Math.max(
      0,
      Math.floor(Number.isFinite(state?.position) ? (state?.position as number) / 1000 : 0),
    )

    setPlayback((current) => ({
      ...current,
      artist: artist || current.artist,
      deviceLabel: sdkDeviceIdRef.current ? "Sarah Audio" : current.deviceLabel,
      durationSec,
      hasTrack: true,
      isPlaying: !state?.paused,
      progressSec,
      title,
      volumeLabel: current.volumeLabel,
    }))
    setStatusText(
      `${sdkDeviceIdRef.current ? "Sarah Audio" : "Spotify"} • ${!state?.paused ? "Playing" : "Paused"}`,
    )
  }, [])

  const refreshFromWebSdk = useCallback(async () => {
    const sdkPlayer = sdkPlayerRef.current
    if (!sdkPlayer?.getCurrentState) {
      setStatusText("Sarah Audio refresh is unavailable.")
      return
    }

    try {
      const state = await sdkPlayer.getCurrentState()
      applyWebPlaybackState(state)
    } catch (error) {
      setStatusText(toErrorMessage(error, "Failed to refresh Sarah Audio state."))
    }
  }, [applyWebPlaybackState])

  const runTransportAction = useCallback(
    async (action: "next" | "pause" | "play" | "prev" | "stop") => {
      const toolByAction: Record<typeof action, string> = {
        next: "skipToNext",
        pause: "pausePlayback",
        play: "resumePlayback",
        prev: "skipToPrevious",
        stop: "pausePlayback",
      }

      setIsActionPending(true)
      try {
        const args = sdkDeviceIdRef.current ? { deviceId: sdkDeviceIdRef.current } : {}
        const { isError, text } = await runSpotifyTool(toolByAction[action], args)
        if (isError) {
          setStatusText(text)
        }
      } catch (error) {
        setStatusText(toErrorMessage(error, "Spotify command failed."))
      } finally {
        setIsActionPending(false)
      }
    },
    [runSpotifyTool],
  )

  const handleAdjustVolume = useCallback(
    async (adjustment: number) => {
      setIsActionPending(true)
      try {
        const args = sdkDeviceIdRef.current
          ? { adjustment, deviceId: sdkDeviceIdRef.current }
          : { adjustment }
        const { isError, text } = await runSpotifyTool("adjustVolume", args)
        if (isError) {
          setStatusText(text)
        }
      } catch (error) {
        setStatusText(toErrorMessage(error, "Failed to adjust Spotify volume."))
      } finally {
        setIsActionPending(false)
      }
    },
    [runSpotifyTool],
  )

  const handleSetVolume = useCallback(
    async (volumePercent: number) => {
      setIsActionPending(true)
      try {
        const args = sdkDeviceIdRef.current
          ? { volumePercent, deviceId: sdkDeviceIdRef.current }
          : { volumePercent }
        const { isError, text } = await runSpotifyTool("setVolume", args)
        if (isError) {
          setStatusText(text)
        }
      } catch (error) {
        setStatusText(toErrorMessage(error, "Failed to set Spotify volume."))
      } finally {
        setIsActionPending(false)
      }
    },
    [runSpotifyTool],
  )

  const handleSeekTo = useCallback(
    async (targetSec: number) => {
      if (playback.durationSec <= 0) {
        return
      }

      const safeSec = clamp(Math.floor(targetSec), 0, playback.durationSec)
      const positionMs = safeSec * 1000
      setPlayback((current) => ({
        ...current,
        progressSec: safeSec,
      }))

      setIsActionPending(true)
      try {
        const args = sdkDeviceIdRef.current
          ? { positionMs, deviceId: sdkDeviceIdRef.current }
          : { positionMs }
        const { isError, text } = await runSpotifyTool("seekToPosition", args)
        if (isError) {
          const sdkPlayer = sdkPlayerRef.current
          if (sdkPlayer?.seek) {
            await sdkPlayer.seek(positionMs)
          } else {
            setStatusText(text)
          }
        }
      } catch (error) {
        try {
          const sdkPlayer = sdkPlayerRef.current
          if (sdkPlayer?.seek) {
            await sdkPlayer.seek(positionMs)
          } else {
            throw error
          }
        } catch (fallbackError) {
          setStatusText(toErrorMessage(fallbackError, "Failed to seek playback."))
        }
      } finally {
        setIsActionPending(false)
      }
    },
    [playback.durationSec, runSpotifyTool],
  )

  const handleProgressSeek = useCallback(
    (event: { clientX: number }) => {
      if (playback.durationSec <= 0) {
        return
      }

      const rect = progressTrackRef.current?.getBoundingClientRect()
      if (!rect || rect.width <= 0) {
        return
      }

      const ratio = clamp((event.clientX - rect.left) / rect.width, 0, 1)
      const nextSec = Math.floor(ratio * playback.durationSec)
      void handleSeekTo(nextSec)
    },
    [handleSeekTo, playback.durationSec],
  )

  useEffect(() => {
    if (!isVisible || sdkPlayerRef.current) {
      return
    }

    let disposed = false
    let localPlayer: SpotifyWebPlaybackPlayer | null = null

    const attachPlayer = async () => {
      try {
        await getSpotifySdkToken()
        await ensureSpotifyWebSdkLoaded()

        if (disposed || !window.Spotify?.Player) {
          return
        }

        const player = new window.Spotify.Player({
          name: "Sarah Audio Player",
          volume: 0.8,
          getOAuthToken: (callback) => {
            void getSpotifySdkToken()
              .then((token) => callback(token))
              .catch((error) => {
                setStatusText(
                  toErrorMessage(
                    error,
                    "Failed to refresh Spotify token. Run OAuth again from MCP Marketplace.",
                  ),
                )
                callback("")
              })
          },
        })

        player.addListener("ready", (payload: unknown) => {
          const deviceId =
            typeof payload === "object" &&
            payload !== null &&
            "device_id" in payload &&
            typeof (payload as { device_id: unknown }).device_id === "string"
              ? (payload as { device_id: string }).device_id
              : ""

          sdkDeviceIdRef.current = deviceId
          if (typeof window !== "undefined") {
            if (deviceId) {
              window.localStorage.setItem(SPOTIFY_WEB_DEVICE_STORAGE_KEY, deviceId)
            } else {
              window.localStorage.removeItem(SPOTIFY_WEB_DEVICE_STORAGE_KEY)
            }
          }
          setStatusText(
            deviceId
              ? "Sarah Audio is online. Playback can run here without Spotify desktop."
              : "Web player is online, but no device id was returned by Spotify.",
          )
          void refreshFromWebSdk()
        })

        player.addListener("not_ready", () => {
          sdkDeviceIdRef.current = ""
          if (typeof window !== "undefined") {
            window.localStorage.removeItem(SPOTIFY_WEB_DEVICE_STORAGE_KEY)
          }
          setStatusText("Sarah Audio went offline. Re-open the audio window to reconnect.")
        })

        player.addListener("authentication_error", (payload: unknown) => {
          const message =
            typeof payload === "object" &&
            payload !== null &&
            "message" in payload &&
            typeof (payload as { message: unknown }).message === "string"
              ? (payload as { message: string }).message
              : "Authentication failed."
          setStatusText(
            `Spotify auth error: ${message}. Run OAuth again and ensure streaming scope is included.`,
          )
        })

        player.addListener("account_error", (payload: unknown) => {
          const message =
            typeof payload === "object" &&
            payload !== null &&
            "message" in payload &&
            typeof (payload as { message: unknown }).message === "string"
              ? (payload as { message: string }).message
              : "Account error."
          setStatusText(`Spotify account error: ${message}`)
        })

        player.addListener("initialization_error", (payload: unknown) => {
          const message =
            typeof payload === "object" &&
            payload !== null &&
            "message" in payload &&
            typeof (payload as { message: unknown }).message === "string"
              ? (payload as { message: string }).message
              : "Initialization error."
          setStatusText(`Spotify SDK init error: ${message}`)
        })

        player.addListener("playback_error", (payload: unknown) => {
          const message =
            typeof payload === "object" &&
            payload !== null &&
            "message" in payload &&
            typeof (payload as { message: unknown }).message === "string"
              ? (payload as { message: string }).message
              : "Playback error."
          setStatusText(`Spotify playback error: ${message}`)
        })

        player.addListener("player_state_changed", (payload: unknown) => {
          const state =
            payload && typeof payload === "object"
              ? (payload as SpotifyWebPlaybackState)
              : null
          applyWebPlaybackState(state)
        })

        sdkPlayerRef.current = player
        localPlayer = player
        const connected = await player.connect()
        if (disposed) {
          player.disconnect()
          return
        }

        if (!connected) {
          setStatusText(
            "Sarah Audio did not connect. Run OAuth again and confirm Premium + streaming scope.",
          )
        }
      } catch (error) {
        setStatusText(
          toErrorMessage(
            error,
            "Failed to initialize Sarah Audio. Re-run OAuth from MCP Marketplace.",
          ),
        )
      }
    }

    void attachPlayer()

    return () => {
      disposed = true
      if (localPlayer) {
        localPlayer.disconnect()
      }
      if (sdkPlayerRef.current === localPlayer) {
        sdkPlayerRef.current = null
      }
      sdkDeviceIdRef.current = ""
      if (typeof window !== "undefined") {
        window.localStorage.removeItem(SPOTIFY_WEB_DEVICE_STORAGE_KEY)
      }
    }
  }, [applyWebPlaybackState, getSpotifySdkToken, isVisible, refreshFromWebSdk])

  useEffect(() => {
    if (!isVisible || !autoplayOnOpen) {
      autoplayAttemptedRef.current = false
      return
    }

    if (autoplayAttemptedRef.current) {
      return
    }

    autoplayAttemptedRef.current = true
    if (!playback.isPlaying) {
      void runTransportAction("play")
    }
  }, [autoplayOnOpen, isVisible, playback.isPlaying, runTransportAction])

  useEffect(() => {
    onPlayingChange?.(playback.isPlaying)
  }, [onPlayingChange, playback.isPlaying])

  useEffect(() => {
    if (!playback.isPlaying || playback.durationSec <= 0) {
      return
    }

    const tickId = window.setInterval(() => {
      setPlayback((current) => {
        if (!current.isPlaying || current.durationSec <= 0) {
          return current
        }

        return {
          ...current,
          progressSec: Math.min(current.durationSec, current.progressSec + 1),
        }
      })
    }, 1000)

    return () => window.clearInterval(tickId)
  }, [playback.durationSec, playback.isPlaying])

  useEffect(() => {
    let unlisten: null | (() => void) = null
    let disposed = false

    void listen<{ action: string }>("sarah://audio-control", (event) => {
      const action = event.payload?.action?.trim().toLowerCase()
      if (!action) {
        return
      }

      if (action === "play" || action === "pause" || action === "stop" || action === "next" || action === "prev") {
        void runTransportAction(action)
      }
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
  }, [runTransportAction])

  const isDraggable = draggable
  const useFramerDrag = isDraggable && !windowTitlebarDrag
  const progressPercent =
    playback.durationSec > 0
      ? Math.min(100, Math.max(0, (playback.progressSec / playback.durationSec) * 100))
      : 0
  const chipLabel = playback.volumeLabel !== "--" ? playback.volumeLabel : playback.isPlaying ? "Playing" : "Paused"

  return (
    <AnimatePresence initial={false}>
      {isVisible && (
        <motion.section
          className="sarah-audio-dock sarah-audio-dock--floating"
          initial={{ opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.985 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          drag={useFramerDrag}
          dragControls={useFramerDrag ? dragControls : undefined}
          dragListener={false}
          dragMomentum={false}
          dragElastic={0.12}
          aria-label="Sarah Audio player"
          data-tauri-disable-drag-region={windowTitlebarDrag ? undefined : "true"}
          ref={dockRef}
        >
          <div
            className={cn(
              "sarah-audio-dock__header",
              "sarah-audio-titlebar",
              useFramerDrag && "sarah-audio-dock__drag",
            )}
            data-tauri-drag-region={windowTitlebarDrag ? "" : undefined}
            onPointerDown={(event) => {
              if (useFramerDrag) {
                dragControls.start(event)
              }
            }}
          >
            <div className="sarah-audio-dock__title">
              <span className="sarah-audio-dock__icon">
                <Music2 className="size-3.5" />
              </span>
              Sarah Audio
            </div>
            <div className="sarah-audio-dock__actions" data-tauri-disable-drag-region="true">
              <Button
                variant="ghost"
                size="icon-sm"
                className="sarah-audio-control"
                disabled={isActionPending}
                onClick={() => void runTransportAction("prev")}
                aria-label="Previous track"
              >
                <SkipBack className="size-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                className="sarah-audio-control"
                disabled={isActionPending}
                onClick={() => void runTransportAction("next")}
                aria-label="Next track"
              >
                <SkipForward className="size-4" />
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    className="sarah-audio-control"
                    aria-label="Playback controls"
                  >
                    <Gauge className="size-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="sarah-audio-menu sarah-audio-menu--settings">
                  <DropdownMenuItem
                    className="sarah-audio-menu__item sarah-audio-menu__item--settings"
                    onClick={() => void refreshFromWebSdk()}
                  >
                    Refresh now
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className="sarah-audio-menu__item sarah-audio-menu__item--settings"
                    onClick={() => void handleSeekTo(playback.progressSec - 10)}
                  >
                    Seek -10s
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className="sarah-audio-menu__item sarah-audio-menu__item--settings"
                    onClick={() => void handleSeekTo(playback.progressSec + 10)}
                  >
                    Seek +10s
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className="sarah-audio-menu__item sarah-audio-menu__item--settings"
                    onClick={() => void handleAdjustVolume(10)}
                  >
                    Volume +10
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className="sarah-audio-menu__item sarah-audio-menu__item--settings"
                    onClick={() => void handleAdjustVolume(-10)}
                  >
                    Volume -10
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className="sarah-audio-menu__item sarah-audio-menu__item--settings"
                    onClick={() => void handleSetVolume(25)}
                  >
                    Set 25%
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className="sarah-audio-menu__item sarah-audio-menu__item--settings"
                    onClick={() => void handleSetVolume(50)}
                  >
                    Set 50%
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className="sarah-audio-menu__item sarah-audio-menu__item--settings"
                    onClick={() => void handleSetVolume(75)}
                  >
                    Set 75%
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    className="sarah-audio-menu__item sarah-audio-menu__item--settings"
                    onClick={() => void handleSetVolume(100)}
                  >
                    Set 100%
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
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
                mode="vu"
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
                {playback.title}
              </div>
              <div className="sarah-audio-artist">
                {playback.artist}
                <span className="sarah-audio-chip">{chipLabel}</span>
              </div>
              <div className="sarah-audio-progress-row">
                <span className="sarah-audio-progress-time">{formatClock(playback.progressSec)}</span>
                <div
                  className="sarah-audio-progress-track"
                  role="slider"
                  tabIndex={0}
                  aria-label="Playback progress"
                  aria-valuemin={0}
                  aria-valuemax={Math.max(playback.durationSec, 0)}
                  aria-valuenow={Math.max(playback.progressSec, 0)}
                  aria-valuetext={`${formatClock(playback.progressSec)} of ${formatClock(playback.durationSec)}`}
                  ref={progressTrackRef}
                  onClick={handleProgressSeek}
                  onKeyDown={(event) => {
                    if (event.key === "ArrowLeft") {
                      event.preventDefault()
                      void handleSeekTo(playback.progressSec - 5)
                      return
                    }
                    if (event.key === "ArrowRight") {
                      event.preventDefault()
                      void handleSeekTo(playback.progressSec + 5)
                      return
                    }
                    if (event.key === "Home") {
                      event.preventDefault()
                      void handleSeekTo(0)
                      return
                    }
                    if (event.key === "End") {
                      event.preventDefault()
                      void handleSeekTo(playback.durationSec)
                    }
                  }}
                >
                  <span
                    className="sarah-audio-progress-range"
                    style={{ width: `${progressPercent}%` }}
                  />
                </div>
                <span className="sarah-audio-progress-time">{formatClock(playback.durationSec)}</span>
              </div>
              <div className="sarah-audio-status-note">{statusText}</div>
            </div>

            <div className="sarah-audio-controls">
              <Button
                type="button"
                size="icon"
                className="sarah-audio-play"
                onClick={() => void runTransportAction(playback.isPlaying ? "pause" : "play")}
                aria-label={playback.isPlaying ? "Pause playback" : "Resume playback"}
                disabled={isActionPending}
              >
                {isActionPending ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : playback.isPlaying ? (
                  <Pause className="size-4 fill-current" />
                ) : (
                  <Play className="size-4 fill-current" />
                )}
              </Button>
            </div>
          </div>
        </motion.section>
      )}
    </AnimatePresence>
  )
}

export { SpotifyAudioPlayer }
