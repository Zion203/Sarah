import * as React from "react"
import { useEffect, useMemo, useRef, useState } from "react"

import { cn } from "@/lib/utils"

export type Frame = number[][]
type MatrixMode = "default" | "vu" | "audio"

interface CellPosition {
  x: number
  y: number
}

interface MatrixProps extends React.HTMLAttributes<HTMLDivElement> {
  rows: number
  cols: number
  pattern?: Frame
  frames?: Frame[]
  fps?: number
  autoplay?: boolean
  loop?: boolean
  size?: number
  gap?: number
  palette?: {
    on: string
    off: string
  }
  brightness?: number
  ariaLabel?: string
  onFrame?: (index: number) => void
  mode?: MatrixMode
  levels?: number[]
}

function clamp(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function averageLevel(levels: number[] | undefined): number {
  if (!levels || levels.length === 0) return 0
  const total = levels.reduce((sum, value) => sum + value, 0)
  return clamp(total / levels.length)
}

function audioReactiveFrame(
  rows: number,
  cols: number,
  levels: number[] | undefined,
  phase: number,
  patternIndex: number
): Frame {
  const frame = emptyFrame(rows, cols)
  const avg = averageLevel(levels)
  const centerX = (cols - 1) / 2
  const centerY = (rows - 1) / 2
  const maxRadius = Math.min(rows, cols) * 0.48
  const basePulse = (Math.sin(phase * 1.2) * 0.5 + 0.5) * (0.4 + avg * 0.6)

  const stamp = (row: number, col: number, value: number) => {
    if (row < 0 || row >= rows || col < 0 || col >= cols) return
    frame[row][col] = Math.max(frame[row][col], value)
  }

  if (patternIndex === 0) {
    // Organic: Ripple rings
    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        const dx = col - centerX
        const dy = row - centerY
        const dist = Math.sqrt(dx * dx + dy * dy)
        const ripplePhase = phase * (0.9 + avg * 0.6) + dist * 0.75
        const ripple = Math.sin(ripplePhase) * 0.5 + 0.5
        const rippleMask = Math.max(0, ripple - 0.7)
        if (rippleMask > 0) {
          stamp(row, col, rippleMask * (0.45 + avg * 0.75))
        }

        const ring1 = Math.abs(dist - maxRadius * (0.28 + basePulse * 0.3))
        const ring2 = Math.abs(dist - maxRadius * (0.64 + basePulse * 0.24))
        if (ring1 < 0.45 || ring2 < 0.45) {
          stamp(row, col, 0.24 + avg * 0.65)
        }
      }
    }
  } else if (patternIndex === 1) {
    // Geometric: Rotating square frame + diagonals
    const angle = phase * (0.9 + avg * 1.2)
    const cosA = Math.cos(angle)
    const sinA = Math.sin(angle)
    const size = maxRadius * (0.45 + avg * 0.2)
    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        const x = col - centerX
        const y = row - centerY
        const rx = x * cosA - y * sinA
        const ry = x * sinA + y * cosA
        const edge = Math.max(Math.abs(rx), Math.abs(ry))
        if (Math.abs(edge - size) < 0.55) {
          stamp(row, col, 0.45 + avg * 0.5)
        }
        const diag = Math.abs(rx + ry)
        if (diag < 0.7) {
          stamp(row, col, 0.22 + avg * 0.4)
        }
        const diag2 = Math.abs(rx - ry)
        if (diag2 < 0.7) {
          stamp(row, col, 0.22 + avg * 0.4)
        }
      }
    }
  } else if (patternIndex === 2) {
    // Geometric: Spiral arcs tuned to audio
    const spiralSpeed = phase * (1.2 + avg * 1.8)
    const armCount = 3
    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        const dx = col - centerX
        const dy = row - centerY
        const dist = Math.sqrt(dx * dx + dy * dy)
        const angle = Math.atan2(dy, dx)
        const spiral = Math.sin(angle * armCount + dist * 0.55 - spiralSpeed)
        if (spiral > 0.82) {
          stamp(row, col, 0.38 + avg * 0.6)
        } else if (spiral > 0.72) {
          stamp(row, col, 0.22 + avg * 0.35)
        }
      }
    }
  } else if (patternIndex === 3) {
    // Heartbeat: EKG line sweep
    const sweep = (phase * (2.1 + avg * 2.2)) % cols
    const baseline = Math.floor(centerY + Math.sin(phase * 0.6) * 1.2)
    for (let col = 0; col < cols; col++) {
      const pulseOffset = col - sweep
      let amplitude = 0
      if (pulseOffset > -1 && pulseOffset < 1) amplitude = 4.5 + avg * 3
      else if (pulseOffset > 2 && pulseOffset < 3) amplitude = -2.5 - avg * 2
      else if (pulseOffset > 4 && pulseOffset < 5) amplitude = 1.5 + avg * 1.2
      const row = Math.round(baseline - amplitude)
      stamp(row, col, 0.75 + avg * 0.2)
      stamp(row + 1, col, 0.3 + avg * 0.3)
      stamp(row - 1, col, 0.3 + avg * 0.3)
    }
  } else {
    // Tech: Pulse checker + sweep row
    const checkerPhase = Math.sin(phase * (1.1 + avg))
    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        const isEven = (row + col) % 2 === 0
        if (isEven && checkerPhase > 0.2) {
          stamp(row, col, 0.28 + avg * 0.4)
        }
      }
    }
    const sweepRow = Math.floor((phase * (2.2 + avg * 2)) % rows)
    for (let col = 0; col < cols; col++) {
      stamp(sweepRow, col, 0.45 + avg * 0.5)
    }
  }

  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      if (frame[row][col] < 0.18) {
        frame[row][col] = 0
      }
    }
  }

  return frame
}

function ensureFrameSize(frame: Frame, rows: number, cols: number): Frame {
  const result: Frame = []
  for (let r = 0; r < rows; r++) {
    const row = frame[r] || []
    result.push([])
    for (let c = 0; c < cols; c++) {
      result[r][c] = row[c] ?? 0
    }
  }
  return result
}

function useAnimation(
  frames: Frame[] | undefined,
  options: {
    fps: number
    autoplay: boolean
    loop: boolean
    onFrame?: (index: number) => void
  }
): { frameIndex: number; isPlaying: boolean } {
  const [frameIndex, setFrameIndex] = useState(0)
  const [isPlaying, setIsPlaying] = useState(options.autoplay)
  const frameIdRef = useRef<number | undefined>(undefined)
  const lastTimeRef = useRef<number>(0)
  const accumulatorRef = useRef<number>(0)

  useEffect(() => {
    if (!frames || frames.length === 0 || !isPlaying) {
      return
    }

    const frameInterval = 1000 / options.fps

    const animate = (currentTime: number) => {
      if (lastTimeRef.current === 0) {
        lastTimeRef.current = currentTime
      }

      const deltaTime = currentTime - lastTimeRef.current
      lastTimeRef.current = currentTime
      accumulatorRef.current += deltaTime

      if (accumulatorRef.current >= frameInterval) {
        accumulatorRef.current -= frameInterval

        setFrameIndex((prev) => {
          const next = prev + 1
          if (next >= frames.length) {
            if (options.loop) {
              options.onFrame?.(0)
              return 0
            } else {
              setIsPlaying(false)
              return prev
            }
          }
          options.onFrame?.(next)
          return next
        })
      }

      frameIdRef.current = requestAnimationFrame(animate)
    }

    frameIdRef.current = requestAnimationFrame(animate)

    return () => {
      if (frameIdRef.current) {
        cancelAnimationFrame(frameIdRef.current)
      }
    }
  }, [frames, isPlaying, options.fps, options.loop, options.onFrame])

  useEffect(() => {
    setFrameIndex(0)
    setIsPlaying(options.autoplay)
    lastTimeRef.current = 0
    accumulatorRef.current = 0
  }, [frames, options.autoplay])

  return { frameIndex, isPlaying }
}

function emptyFrame(rows: number, cols: number): Frame {
  return Array.from({ length: rows }, () => Array(cols).fill(0))
}

function setPixel(frame: Frame, row: number, col: number, value: number): void {
  if (row >= 0 && row < frame.length && col >= 0 && col < frame[0].length) {
    frame[row][col] = value
  }
}

export const digits: Frame[] = [
  [
    [0, 1, 1, 1, 0],
    [1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0],
  ],
  [
    [0, 0, 1, 0, 0],
    [0, 1, 1, 0, 0],
    [0, 0, 1, 0, 0],
    [0, 0, 1, 0, 0],
    [0, 0, 1, 0, 0],
    [0, 0, 1, 0, 0],
    [0, 1, 1, 1, 0],
  ],
  [
    [0, 1, 1, 1, 0],
    [1, 0, 0, 0, 1],
    [0, 0, 0, 0, 1],
    [0, 0, 0, 1, 0],
    [0, 0, 1, 0, 0],
    [0, 1, 0, 0, 0],
    [1, 1, 1, 1, 1],
  ],
  [
    [0, 1, 1, 1, 0],
    [1, 0, 0, 0, 1],
    [0, 0, 0, 0, 1],
    [0, 0, 1, 1, 0],
    [0, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0],
  ],
  [
    [0, 0, 0, 1, 0],
    [0, 0, 1, 1, 0],
    [0, 1, 0, 1, 0],
    [1, 0, 0, 1, 0],
    [1, 1, 1, 1, 1],
    [0, 0, 0, 1, 0],
    [0, 0, 0, 1, 0],
  ],
  [
    [1, 1, 1, 1, 1],
    [1, 0, 0, 0, 0],
    [1, 1, 1, 1, 0],
    [0, 0, 0, 0, 1],
    [0, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0],
  ],
  [
    [0, 1, 1, 1, 0],
    [1, 0, 0, 0, 0],
    [1, 0, 0, 0, 0],
    [1, 1, 1, 1, 0],
    [1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0],
  ],
  [
    [1, 1, 1, 1, 1],
    [0, 0, 0, 0, 1],
    [0, 0, 0, 1, 0],
    [0, 0, 1, 0, 0],
    [0, 1, 0, 0, 0],
    [0, 1, 0, 0, 0],
    [0, 1, 0, 0, 0],
  ],
  [
    [0, 1, 1, 1, 0],
    [1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0],
    [1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0],
  ],
  [
    [0, 1, 1, 1, 0],
    [1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [0, 1, 1, 1, 1],
    [0, 0, 0, 0, 1],
    [0, 0, 0, 0, 1],
    [0, 1, 1, 1, 0],
  ],
]

export const chevronLeft: Frame = [
  [0, 0, 0, 1, 0],
  [0, 0, 1, 0, 0],
  [0, 1, 0, 0, 0],
  [0, 0, 1, 0, 0],
  [0, 0, 0, 1, 0],
]

export const chevronRight: Frame = [
  [0, 1, 0, 0, 0],
  [0, 0, 1, 0, 0],
  [0, 0, 0, 1, 0],
  [0, 0, 1, 0, 0],
  [0, 1, 0, 0, 0],
]

export const loader: Frame[] = (() => {
  const frames: Frame[] = []
  const size = 7
  const center = 3
  const radius = 2.5

  for (let frame = 0; frame < 12; frame++) {
    const f = emptyFrame(size, size)
    for (let i = 0; i < 8; i++) {
      const angle = (frame / 12) * Math.PI * 2 + (i / 8) * Math.PI * 2
      const x = Math.round(center + Math.cos(angle) * radius)
      const y = Math.round(center + Math.sin(angle) * radius)
      const brightness = 1 - i / 10
      setPixel(f, y, x, Math.max(0.2, brightness))
    }
    frames.push(f)
  }

  return frames
})()

export const pulse: Frame[] = (() => {
  const frames: Frame[] = []
  const size = 7
  const center = 3

  for (let frame = 0; frame < 16; frame++) {
    const f = emptyFrame(size, size)
    const phase = (frame / 16) * Math.PI * 2
    const intensity = (Math.sin(phase) + 1) / 2

    setPixel(f, center, center, 1)

    const radius = Math.floor((1 - intensity) * 3) + 1
    for (let dy = -radius; dy <= radius; dy++) {
      for (let dx = -radius; dx <= radius; dx++) {
        const dist = Math.sqrt(dx * dx + dy * dy)
        if (Math.abs(dist - radius) < 0.7) {
          setPixel(f, center + dy, center + dx, intensity * 0.6)
        }
      }
    }

    frames.push(f)
  }

  return frames
})()

export function vu(rows: number, columns: number, levels: number[]): Frame {
  const frame = emptyFrame(rows, columns)

  for (let col = 0; col < Math.min(columns, levels.length); col++) {
    const level = Math.max(0, Math.min(1, levels[col]))
    const height = Math.floor(level * rows)

    for (let row = 0; row < rows; row++) {
      const rowFromBottom = rows - 1 - row
      if (rowFromBottom < height) {
        let brightness = 1
        if (row < rows * 0.3) {
          brightness = 1
        } else if (row < rows * 0.6) {
          brightness = 0.8
        } else {
          brightness = 0.6
        }
        frame[row][col] = brightness
      }
    }
  }

  return frame
}

export const wave: Frame[] = (() => {
  const frames: Frame[] = []
  const rows = 7
  const cols = 7

  for (let frame = 0; frame < 24; frame++) {
    const f = emptyFrame(rows, cols)
    const phase = (frame / 24) * Math.PI * 2

    for (let col = 0; col < cols; col++) {
      const colPhase = (col / cols) * Math.PI * 2
      const height = Math.sin(phase + colPhase) * 2.5 + 3.5
      const row = Math.floor(height)

      if (row >= 0 && row < rows) {
        setPixel(f, row, col, 1)
        const frac = height - row
        if (row > 0) setPixel(f, row - 1, col, 1 - frac)
        if (row < rows - 1) setPixel(f, row + 1, col, frac)
      }
    }

    frames.push(f)
  }

  return frames
})()

export const snake: Frame[] = (() => {
  const frames: Frame[] = []
  const rows = 7
  const cols = 7
  const path: Array<[number, number]> = []

  let x = 0
  let y = 0
  let dx = 1
  let dy = 0

  const visited = new Set<string>()
  while (path.length < rows * cols) {
    path.push([y, x])
    visited.add(`${y},${x}`)

    const nextX = x + dx
    const nextY = y + dy

    if (
      nextX >= 0 &&
      nextX < cols &&
      nextY >= 0 &&
      nextY < rows &&
      !visited.has(`${nextY},${nextX}`)
    ) {
      x = nextX
      y = nextY
    } else {
      const newDx = -dy
      const newDy = dx
      dx = newDx
      dy = newDy

      const nextX = x + dx
      const nextY = y + dy

      if (
        nextX >= 0 &&
        nextX < cols &&
        nextY >= 0 &&
        nextY < rows &&
        !visited.has(`${nextY},${nextX}`)
      ) {
        x = nextX
        y = nextY
      } else {
        break
      }
    }
  }

  const snakeLength = 5
  for (let frame = 0; frame < path.length; frame++) {
    const f = emptyFrame(rows, cols)

    for (let i = 0; i < snakeLength; i++) {
      const idx = frame - i
      if (idx >= 0 && idx < path.length) {
        const [y, x] = path[idx]
        const brightness = 1 - i / snakeLength
        setPixel(f, y, x, brightness)
      }
    }

    frames.push(f)
  }

  return frames
})()

export const Matrix = React.forwardRef<HTMLDivElement, MatrixProps>(
  (
    {
      rows,
      cols,
      pattern,
      frames,
      fps = 12,
      autoplay = true,
      loop = true,
      size = 10,
      gap = 2,
      palette = {
        on: "currentColor",
        off: "var(--muted-foreground)",
      },
      brightness = 1,
      ariaLabel,
      onFrame,
      mode = "default",
      levels,
      className,
      ...props
    },
    ref
  ) => {
    const [phase, setPhase] = useState(0)
    const [patternIndex, setPatternIndex] = useState(0)

    useEffect(() => {
      if (mode !== "audio") return
      let raf = 0
      let last = performance.now()

      const tick = (now: number) => {
        const delta = now - last
        last = now
        const avg = averageLevel(levels)
        const speed = 0.0015 + avg * 0.0035
        setPhase((current) => current + delta * speed)
        raf = requestAnimationFrame(tick)
      }

      raf = requestAnimationFrame(tick)
      return () => cancelAnimationFrame(raf)
    }, [mode, levels])

    useEffect(() => {
      if (mode !== "audio") return
      const intervalMs = 5200
      const timer = window.setInterval(() => {
        setPatternIndex((current) => (current + 1) % 5)
      }, intervalMs)
      return () => window.clearInterval(timer)
    }, [mode])

    const { frameIndex } = useAnimation(frames, {
      fps,
      autoplay: autoplay && !pattern,
      loop,
      onFrame,
    })

    const currentFrame = useMemo(() => {
      if (mode === "vu" && levels && levels.length > 0) {
        return ensureFrameSize(vu(rows, cols, levels), rows, cols)
      }

      if (mode === "audio") {
        return ensureFrameSize(
          audioReactiveFrame(rows, cols, levels, phase, patternIndex),
          rows,
          cols
        )
      }

      if (pattern) {
        return ensureFrameSize(pattern, rows, cols)
      }

      if (frames && frames.length > 0) {
        return ensureFrameSize(frames[frameIndex] || frames[0], rows, cols)
      }

      return ensureFrameSize([], rows, cols)
    }, [pattern, frames, frameIndex, rows, cols, mode, levels])

    const cellPositions = useMemo(() => {
      const positions: CellPosition[][] = []

      for (let row = 0; row < rows; row++) {
        positions[row] = []
        for (let col = 0; col < cols; col++) {
          positions[row][col] = {
            x: col * (size + gap),
            y: row * (size + gap),
          }
        }
      }

      return positions
    }, [rows, cols, size, gap])

    const svgDimensions = useMemo(() => {
      return {
        width: cols * (size + gap) - gap,
        height: rows * (size + gap) - gap,
      }
    }, [rows, cols, size, gap])

    const isAnimating = !pattern && frames && frames.length > 0

    return (
      <div
        ref={ref}
        role="img"
        aria-label={ariaLabel ?? "matrix display"}
        aria-live={isAnimating ? "polite" : undefined}
        className={cn("relative inline-block", className)}
        style={
          {
            "--matrix-on": palette.on,
            "--matrix-off": palette.off,
            "--matrix-gap": `${gap}px`,
            "--matrix-size": `${size}px`,
          } as React.CSSProperties
        }
        {...props}
      >
        <svg
          width={svgDimensions.width}
          height={svgDimensions.height}
          viewBox={`0 0 ${svgDimensions.width} ${svgDimensions.height}`}
          xmlns="http://www.w3.org/2000/svg"
          className="block"
          style={{ overflow: "visible" }}
        >
          <defs>
            <radialGradient id="matrix-pixel-on" cx="50%" cy="50%" r="50%">
              <stop offset="0%" stopColor="var(--matrix-on)" stopOpacity="1" />
              <stop
                offset="70%"
                stopColor="var(--matrix-on)"
                stopOpacity="0.85"
              />
              <stop
                offset="100%"
                stopColor="var(--matrix-on)"
                stopOpacity="0.6"
              />
            </radialGradient>

            <radialGradient id="matrix-pixel-off" cx="50%" cy="50%" r="50%">
              <stop
                offset="0%"
                stopColor="var(--muted-foreground)"
                stopOpacity="1"
              />
              <stop
                offset="100%"
                stopColor="var(--muted-foreground)"
                stopOpacity="0.7"
              />
            </radialGradient>

            <filter
              id="matrix-glow"
              x="-50%"
              y="-50%"
              width="200%"
              height="200%"
            >
              <feGaussianBlur stdDeviation="2" result="blur" />
              <feComposite in="SourceGraphic" in2="blur" operator="over" />
            </filter>
          </defs>

          <style>
            {`
              .matrix-pixel {
                transition: opacity 300ms ease-out, transform 150ms ease-out;
                transform-origin: center;
                transform-box: fill-box;
              }
              .matrix-pixel-active {
                filter: url(#matrix-glow);
              }
            `}
          </style>

          {currentFrame.map((row, rowIndex) =>
            row.map((value, colIndex) => {
              const pos = cellPositions[rowIndex]?.[colIndex]
              if (!pos) return null

              const opacity = clamp(brightness * value)
              const isActive = opacity > 0.5
              const isOn = opacity > 0.05
              const fill = isOn
                ? "url(#matrix-pixel-on)"
                : "url(#matrix-pixel-off)"

              const scale = isActive ? 1.1 : 1
              const radius = (size / 2) * 0.9

              return (
                <circle
                  key={`${rowIndex}-${colIndex}`}
                  className={cn(
                    "matrix-pixel",
                    isActive && "matrix-pixel-active",
                    !isOn && "opacity-20 dark:opacity-[0.1]"
                  )}
                  cx={pos.x + size / 2}
                  cy={pos.y + size / 2}
                  r={radius}
                  fill={fill}
                  opacity={isOn ? opacity : 0.1}
                  style={{
                    transform: `scale(${scale})`,
                  }}
                />
              )
            })
          )}
        </svg>
      </div>
    )
  }
)

Matrix.displayName = "Matrix"
