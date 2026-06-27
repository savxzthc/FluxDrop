import { useEffect, useState } from "react";
import type { AppSettings } from "../lib/api";

type ThemePreference = AppSettings["theme"];

function storedThemePreference(): ThemePreference {
  const storedTheme = window.localStorage.getItem("fluxdrop-theme");
  return storedTheme === "dark" || storedTheme === "light" || storedTheme === "system" ? storedTheme : "system";
}

export function useThemePreference() {
  const [systemDark, setSystemDark] = useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);
  const [themePreference, setThemePreference] = useState<ThemePreference>(storedThemePreference);
  const resolvedTheme =
    themePreference === "dark" || themePreference === "light"
      ? themePreference
      : systemDark
        ? "dark"
        : "light";

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const updateSystemTheme = (event: MediaQueryListEvent) => setSystemDark(event.matches);
    setSystemDark(query.matches);
    query.addEventListener("change", updateSystemTheme);
    return () => query.removeEventListener("change", updateSystemTheme);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedTheme;
    document.documentElement.style.colorScheme = resolvedTheme;
    window.localStorage.setItem("fluxdrop-theme", themePreference);
  }, [resolvedTheme, themePreference]);

  return { resolvedTheme, setThemePreference, themePreference };
}
