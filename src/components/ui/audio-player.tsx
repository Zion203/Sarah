"use client"

import {
  ComponentProps,
  createContext,
  HTMLProps,
  ReactNode,
  RefObject,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react"
import * as SliderPrimitive from "@radix-ui/react-slider"
import { Check, Gauge, PauseIcon, PlayIcon } from "lucide-react"

import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

function formatTime(seconds: number) {
  const safe = Number.isFinite(seconds) ? Math.max(0, seconds) : 0
  const hrs = Math.floor(safe / 3600)
  const mins = Math.floor((safe % 3600) / 60)
  const secs = Math.floor(safe % 60)

  const formattedMins = mins < 10 ? `0${mins}` : mins
  const formattedSecs = secs < 10 ? `0${secs}` : secs

  return hrs > 0
    ? `${hrs}:${formattedMins}:${formattedSecs}`
    : `${mins}:${formattedSecs}`
}

interface AudioPlayerItem<TData = unknown> {
  id: string | number
  src: string
  data?: TData
}

interface AudioPlayerApi<TData = unknown> {
  ref: RefObject<HTMLAudioElement | null>
  activeItem: AudioPlayerItem<TData> | null
  duration: number | undefined
  isPlaying: boolean
  isBuffering: boolean
  playbackRate: number
  isItemActive: (id: string | number | null) => boolean
  setActiveItem: (item: AudioPlayerItem<TData> | null) => void
  play: (item?: AudioPlayerItem<TData> | null) => Promise<void>
  pause: () => void
  seek: (time: number) => void
  setPlaybackRate: (rate: number) => void
}

const AudioPlayerContext = createContext<AudioPlayerApi<unknown> | null>(null)

export function useAudioPlayer<TData = unknown>(): AudioPlayerApi<TData> {
  const api = useContext(AudioPlayerContext) as AudioPlayerApi<TData> | null
  if (!api) {
    throw new Error(
      "useAudioPlayer cannot be called outside of AudioPlayerProvider"
    )
  }
  return api
}

const AudioPlayerTimeContext = createContext<number | null>(null)

export const useAudioPlayerTime = () => {
  const time = useContext(AudioPlayerTimeContext)
  if (time === null) {
    throw new Error(
      "useAudioPlayerTime cannot be called outside of AudioPlayerProvider"
    )
  }
  return time
}

export function AudioPlayerProvider<TData = unknown>({
  children,
}: {
  children: ReactNode
}) {
  const audioRef = useRef<HTMLAudioElement>(null)
  const [activeItem, setActiveItemState] = useState<AudioPlayerItem<TData> | null>(
    null
  )
  const [time, setTime] = useState(0)
  const [duration, setDuration] = useState<number | undefined>(undefined)
  const [isPlaying, setIsPlaying] = useState(false)
  const [isBuffering, setIsBuffering] = useState(false)
  const [playbackRate, setPlaybackRateState] = useState(1)

  const setActiveItem = useCallback((item: AudioPlayerItem<TData> | null) => {
    const audio = audioRef.current
    setActiveItemState(item)
    if (!audio) return

    if (!item) {
      audio.removeAttribute("src")
      audio.load()
      return
    }

    if (audio.src !== item.src) {
      audio.src = item.src
      audio.load()
    }
  }, [])

  const play = useCallback(
    async (item?: AudioPlayerItem<TData> | null) => {
      const audio = audioRef.current
      if (!audio) return

      if (item !== undefined) {
        setActiveItem(item)
      } else if (activeItem) {
        if (!audio.src || audio.src !== activeItem.src) {
          audio.src = activeItem.src
          audio.load()
        }
      }

      if (audio.ended) {
        audio.currentTime = 0
      }

      await audio.play()
    },
    [activeItem, setActiveItem]
  )

  const pause = useCallback(() => {
    const audio = audioRef.current
    if (!audio) return
    audio.pause()
  }, [])

  const seek = useCallback((nextTime: number) => {
    const audio = audioRef.current
    if (!audio) return
    audio.currentTime = nextTime
  }, [])

  const setPlaybackRate = useCallback((rate: number) => {
    const audio = audioRef.current
    if (!audio) return
    audio.playbackRate = rate
    setPlaybackRateState(rate)
  }, [])

  useEffect(() => {
    const audio = audioRef.current
    if (!audio) return

    const onTime = () => setTime(audio.currentTime)
    const onLoaded = () => setDuration(Number.isFinite(audio.duration) ? audio.duration : undefined)
    const onPlay = () => setIsPlaying(true)
    const onPause = () => setIsPlaying(false)
    const onEnded = () => setIsPlaying(false)
    const onWaiting = () => setIsBuffering(true)
    const onCanPlay = () => setIsBuffering(false)
    const onRateChange = () => setPlaybackRateState(audio.playbackRate)

    audio.addEventListener("timeupdate", onTime)
    audio.addEventListener("loadedmetadata", onLoaded)
    audio.addEventListener("durationchange", onLoaded)
    audio.addEventListener("play", onPlay)
    audio.addEventListener("pause", onPause)
    audio.addEventListener("ended", onEnded)
    audio.addEventListener("waiting", onWaiting)
    audio.addEventListener("canplay", onCanPlay)
    audio.addEventListener("ratechange", onRateChange)

    return () => {
      audio.removeEventListener("timeupdate", onTime)
      audio.removeEventListener("loadedmetadata", onLoaded)
      audio.removeEventListener("durationchange", onLoaded)
      audio.removeEventListener("play", onPlay)
      audio.removeEventListener("pause", onPause)
      audio.removeEventListener("ended", onEnded)
      audio.removeEventListener("waiting", onWaiting)
      audio.removeEventListener("canplay", onCanPlay)
      audio.removeEventListener("ratechange", onRateChange)
    }
  }, [])

  const api = useMemo<AudioPlayerApi<TData>>(
    () => ({
      ref: audioRef,
      duration,
      isPlaying,
      isBuffering,
      activeItem,
      playbackRate,
      isItemActive: (id: string | number | null) => activeItem?.id === id,
      setActiveItem,
      play,
      pause,
      seek,
      setPlaybackRate,
    }),
    [duration, isPlaying, isBuffering, activeItem, playbackRate, setActiveItem, play, pause, seek, setPlaybackRate]
  )

  return (
    <AudioPlayerContext.Provider value={api as AudioPlayerApi<unknown>}>
      <AudioPlayerTimeContext.Provider value={time}>
        <audio
          ref={audioRef}
          className="sarah-audio-element"
          preload="auto"
          crossOrigin="anonymous"
        />
        {children}
      </AudioPlayerTimeContext.Provider>
    </AudioPlayerContext.Provider>
  )
}

export const AudioPlayerProgress = ({
  ...otherProps
}: Omit<
  ComponentProps<typeof SliderPrimitive.Root>,
  "min" | "max" | "value"
>) => {
  const player = useAudioPlayer()
  const time = useAudioPlayerTime()

  return (
    <SliderPrimitive.Root
      {...otherProps}
      value={[time]}
      onValueChange={(vals) => {
        player.seek(vals[0])
        otherProps.onValueChange?.(vals)
      }}
      min={0}
      max={player.duration ?? 0}
      step={otherProps.step || 0.25}
      className={cn(
        "group/player relative flex h-4 touch-none items-center select-none data-[disabled]:opacity-50",
        otherProps.className
      )}
      disabled={
        player.duration === undefined ||
        !Number.isFinite(player.duration) ||
        Number.isNaN(player.duration)
      }
    >
      <SliderPrimitive.Track
        className="bg-muted relative h-[4px] w-full grow overflow-hidden rounded-full"
        data-slot="slider-track"
      >
        <SliderPrimitive.Range
          className="bg-primary absolute h-full"
          data-slot="slider-range"
        />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb
        className="relative flex h-0 w-0 items-center justify-center opacity-0 group-hover/player:opacity-100 focus-visible:opacity-100 focus-visible:outline-none"
        data-slot="slider-thumb"
      >
        <div className="bg-foreground absolute size-3 rounded-full" />
      </SliderPrimitive.Thumb>
    </SliderPrimitive.Root>
  )
}

export const AudioPlayerTime = ({
  className,
  ...otherProps
}: HTMLProps<HTMLSpanElement>) => {
  const time = useAudioPlayerTime()
  return (
    <span
      {...otherProps}
      className={cn("text-muted-foreground text-sm tabular-nums", className)}
    >
      {formatTime(time)}
    </span>
  )
}

export const AudioPlayerDuration = ({
  className,
  ...otherProps
}: HTMLProps<HTMLSpanElement>) => {
  const player = useAudioPlayer()
  return (
    <span
      {...otherProps}
      className={cn("text-muted-foreground text-sm tabular-nums", className)}
    >
      {player.duration !== null &&
      player.duration !== undefined &&
      !Number.isNaN(player.duration)
        ? formatTime(player.duration)
        : "--:--"}
    </span>
  )
}

interface SpinnerProps {
  className?: string
}

function Spinner({ className }: SpinnerProps) {
  return (
    <div
      className={cn(
        "border-muted border-t-foreground size-3.5 animate-spin rounded-full border-2",
        className
      )}
      role="status"
      aria-label="Loading"
    >
      <span className="sr-only">Loading...</span>
    </div>
  )
}

interface PlayButtonProps extends React.ComponentProps<typeof Button> {
  playing: boolean
  onToggle: () => void
  loading?: boolean
}

const PlayButton = ({
  playing,
  onToggle,
  className,
  onClick,
  loading,
  ...otherProps
}: PlayButtonProps) => {
  return (
    <Button
      {...otherProps}
      onClick={(e) => {
        onToggle()
        onClick?.(e)
      }}
      className={cn("relative", className)}
      aria-label={playing ? "Pause" : "Play"}
      type="button"
    >
      {playing ? (
        <PauseIcon
          className={cn("size-4", loading && "opacity-0")}
          aria-hidden="true"
        />
      ) : (
        <PlayIcon
          className={cn("size-4", loading && "opacity-0")}
          aria-hidden="true"
        />
      )}
      {loading && (
        <div className="absolute inset-0 flex items-center justify-center rounded-[inherit] backdrop-blur-xs">
          <Spinner />
        </div>
      )}
    </Button>
  )
}

export interface AudioPlayerButtonProps<TData = unknown>
  extends React.ComponentProps<typeof Button> {
  item?: AudioPlayerItem<TData>
}

export function AudioPlayerButton<TData = unknown>({
  item,
  ...otherProps
}: AudioPlayerButtonProps<TData>) {
  const player = useAudioPlayer<TData>()

  const toggle = () => {
    const audio = player.ref.current
    const shouldPlay = audio ? audio.paused || audio.ended : !player.isPlaying
    if (shouldPlay) {
      if (item) {
        player.play(item)
      } else {
        player.play()
      }
    } else {
      player.pause()
    }
  }

  return (
    <PlayButton
      {...otherProps}
      playing={item ? player.isItemActive(item.id) && player.isPlaying : player.isPlaying}
      onToggle={toggle}
      loading={player.isBuffering && player.isPlaying}
    />
  )
}

const PLAYBACK_SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2] as const

export interface AudioPlayerSpeedProps
  extends React.ComponentProps<typeof Button> {
  speeds?: readonly number[]
  menuClassName?: string
  open?: boolean
  onOpenChange?: (open: boolean) => void
}

export function AudioPlayerSpeed({
  speeds = PLAYBACK_SPEEDS,
  className,
  variant = "ghost",
  size = "icon",
  menuClassName,
  open,
  onOpenChange,
  ...props
}: AudioPlayerSpeedProps) {
  const player = useAudioPlayer()
  const currentSpeed = player.playbackRate

  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange}>
      <DropdownMenuTrigger asChild>
        <Button
          variant={variant}
          size={size}
          className={cn(className)}
          aria-label="Playback speed"
          {...props}
        >
          <Gauge className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className={cn("min-w-[120px]", menuClassName)}
      >
        {speeds.map((speed) => (
          <DropdownMenuItem
            key={speed}
            onClick={() => player.setPlaybackRate(speed)}
            className="sarah-audio-menu__item sarah-audio-menu__item--settings flex items-center justify-between"
          >
            <span
              className={cn(
                "sarah-audio-menu__speed-label",
                speed === 1 ? "" : "font-mono"
              )}
            >
              {speed === 1 ? "Normal" : `${speed}x`}
            </span>
            {currentSpeed === speed && <Check className="size-4" />}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

export const exampleTracks = [
  {
    id: "0",
    name: "II - 00",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/00.mp3",
  },
  {
    id: "1",
    name: "II - 01",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/01.mp3",
  },
  {
    id: "2",
    name: "II - 02",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/02.mp3",
  },
  {
    id: "3",
    name: "II - 03",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/03.mp3",
  },
  {
    id: "4",
    name: "II - 04",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/04.mp3",
  },
  {
    id: "5",
    name: "II - 05",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/05.mp3",
  },
  {
    id: "6",
    name: "II - 06",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/06.mp3",
  },
  {
    id: "7",
    name: "II - 07",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/07.mp3",
  },
  {
    id: "8",
    name: "II - 08",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/08.mp3",
  },
  {
    id: "9",
    name: "II - 09",
    url: "https://storage.googleapis.com/eleven-public-cdn/audio/ui-elevenlabs-io/09.mp3",
  },
]
