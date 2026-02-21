import { useCallback, useEffect, useMemo, useState } from "react";

export type ThemeMode = "light" | "dark";
export const THEME_STORAGE_KEY = "sarah_theme_mode_v1";

function resolveSystemTheme(): ThemeMode {
  if (typeof window === "undefined") {
    return "light";
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function parseTheme(value: string | null): null | ThemeMode {
  if (value === "light" || value === "dark") {
    return value;
  }

  return null;
}

function applyTheme(theme: ThemeMode) {
  if (typeof document === "undefined") {
    return;
  }

  document.documentElement.classList.toggle("dark", theme === "dark");
}

export function useTheme() {
  const [theme, setTheme] = useState<ThemeMode>(() => {
    if (typeof window === "undefined") {
      return "light";
    }

    const stored = parseTheme(window.localStorage.getItem(THEME_STORAGE_KEY));
    const initialTheme = stored ?? resolveSystemTheme();
    applyTheme(initialTheme);
    return initialTheme;
  });

  useEffect(() => {
    applyTheme(theme);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(THEME_STORAGE_KEY, theme);
    }
  }, [theme]);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key !== THEME_STORAGE_KEY) {
        return;
      }

      const nextTheme = parseTheme(event.newValue);
      if (nextTheme) {
        setTheme(nextTheme);
      }
    };

    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const toggleTheme = useCallback(() => {
    setTheme((current) => (current === "dark" ? "light" : "dark"));
  }, []);

  const isDarkTheme = useMemo(() => theme === "dark", [theme]);

  return {
    isDarkTheme,
    theme,
    toggleTheme,
  };
}
