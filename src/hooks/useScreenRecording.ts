import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ScreenCaptureSurface } from "@/hooks/useAppPreferences";

export interface ScreenRecordingResult {
  durationMs: number;
  endedAtMs: number;
  id: string;
  mimeType: string;
  screenshotPath: null | string;
  startedAtMs: number;
  videoPath: string;
}

export interface StartScreenRecordingResult {
  error?: string;
  ok: boolean;
}

interface NativeStopRecordingPayload {
  durationMs: number;
  endedAtMs: number;
  mimeType: string;
  screenshotPath: null | string;
  startedAtMs: number;
  videoPath: string;
}

function extractErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message || "Screen recording failed.";
  }

  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "Screen recording failed.";
}

export function useScreenRecording() {
  const [elapsedMs, setElapsedMs] = useState(0);
  const [isRecording, setIsRecording] = useState(false);
  const [lastError, setLastError] = useState<null | string>(null);
  const [result, setResult] = useState<null | ScreenRecordingResult>(null);
  const startedAtMsRef = useRef<null | number>(null);
  const timerRef = useRef<null | number>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const clearError = useCallback(() => {
    setLastError(null);
  }, []);

  const startRecording = useCallback(
    async (
      preferredSurface: ScreenCaptureSurface = "screen",
      windowHwnd?: string,
      outputDirectory?: string,
    ): Promise<StartScreenRecordingResult> => {
      if (isRecording) {
        return {
          error: "Screen recording is already running.",
          ok: false,
        };
      }

      clearTimer();
      setLastError(null);
      setResult(null);
      setElapsedMs(0);

      const startedAtMs = Date.now();
      startedAtMsRef.current = startedAtMs;

      try {
        const payload: Record<string, unknown> = {
          surface: preferredSurface,
        };
        if (typeof windowHwnd === "string" && windowHwnd.trim().length > 0) {
          payload.windowHwnd = windowHwnd;
        }
        if (typeof outputDirectory === "string" && outputDirectory.trim().length > 0) {
          payload.outputDirectory = outputDirectory.trim();
        }

        await invoke("start_native_screen_recording", payload);

        setIsRecording(true);
        timerRef.current = window.setInterval(() => {
          if (!startedAtMsRef.current) {
            return;
          }
          setElapsedMs(Math.max(0, Date.now() - startedAtMsRef.current));
        }, 120);

        return { ok: true };
      } catch (error) {
        startedAtMsRef.current = null;
        setIsRecording(false);
        const message = extractErrorMessage(error);
        setLastError(message);
        return {
          error: message,
          ok: false,
        };
      }
    },
    [clearTimer, isRecording],
  );

  const stopRecording = useCallback(() => {
    if (!isRecording) {
      return;
    }

    void (async () => {
      try {
        const payload = await invoke<NativeStopRecordingPayload>("stop_native_screen_recording");
        clearTimer();
        setIsRecording(false);
        startedAtMsRef.current = null;
        setElapsedMs(payload.durationMs);
        setResult({
          durationMs: payload.durationMs,
          endedAtMs: payload.endedAtMs,
          id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
          mimeType: payload.mimeType,
          screenshotPath: payload.screenshotPath,
          startedAtMs: payload.startedAtMs,
          videoPath: payload.videoPath,
        });
      } catch (error) {
        clearTimer();
        setIsRecording(false);
        startedAtMsRef.current = null;
        const message = extractErrorMessage(error);
        setLastError(message);
      }
    })();
  }, [clearTimer, isRecording]);

  useEffect(() => {
    return () => {
      clearTimer();
    };
  }, [clearTimer]);

  return {
    clearError,
    elapsedMs,
    isRecording,
    lastError,
    result,
    startRecording,
    stopRecording,
  };
}
