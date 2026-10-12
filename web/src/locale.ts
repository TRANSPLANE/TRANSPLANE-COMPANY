import { useSyncExternalStore } from "react";

export const supportedLocales = ["zh-CN", "en"] as const;
export type Locale = (typeof supportedLocales)[number];
const preferenceKey = "transplane-company-locale";
export function defaultLocale(languages: readonly string[]): Locale {
  return languages[0]?.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}
function initialLocale(): Locale {
  try {
    const stored = localStorage.getItem(preferenceKey);
    if (stored === "zh-CN" || stored === "en") return stored;
  } catch {
    /* Storage may be disabled. */
  }
  return defaultLocale(
    typeof navigator === "undefined"
      ? ["en"]
      : navigator.languages || [navigator.language],
  );
}
let currentLocale = initialLocale();
const listeners = new Set<() => void>();
export function getLocale(): Locale {
  return currentLocale;
}
export function setLocale(locale: Locale) {
  if (!supportedLocales.includes(locale)) return;
  currentLocale = locale;
  try {
    localStorage.setItem(preferenceKey, locale);
  } catch {
    /* Keep the current in-memory preference. */
  }
  if (typeof document !== "undefined") document.documentElement.lang = locale;
  for (const listener of listeners) listener();
}
export function useLocale(): Locale {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getLocale,
    getLocale,
  );
}
if (typeof document !== "undefined")
  document.documentElement.lang = currentLocale;
if (typeof window !== "undefined")
  window.addEventListener("storage", (event) => {
    if (
      event.key === preferenceKey &&
      (event.newValue === "zh-CN" || event.newValue === "en")
    ) {
      currentLocale = event.newValue;
      document.documentElement.lang = currentLocale;
      for (const listener of listeners) listener();
    }
  });

export function localized<T extends object>(zh: T, en: T): T {
  const nested = new Map<PropertyKey, unknown>();
  return new Proxy(zh, {
    get(target, key) {
      const value = Reflect.get(getLocale() === "en" ? en : target, key);
      if (value && typeof value === "object" && !Array.isArray(value)) {
        if (!nested.has(key))
          nested.set(
            key,
            localized(
              Reflect.get(target, key) as object,
              Reflect.get(en, key) as object,
            ),
          );
        return nested.get(key);
      }
      return value;
    },
  });
}
export function catalogLeaves(value: object, prefix = ""): string[] {
  return Object.entries(value).flatMap(([key, entry]) =>
    entry && typeof entry === "object"
      ? catalogLeaves(entry, `${prefix}${key}.`)
      : [`${prefix}${key}`],
  );
}
