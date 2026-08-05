// Locale registry and selection precedence.
//
// English only for now (openagents#9323 scope). The precedence below is the
// specified order — explicit setting, URL parameter, previous URL locale,
// browser language, default — so adding a second locale is a data change, not
// a mechanism change.

import { makeCatalog, type Catalog } from "./catalog.js";

export const SUPPORTED_LOCALES = ["en"] as const;

export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];

export const DEFAULT_LOCALE: SupportedLocale = "en";

const catalogs: Record<SupportedLocale, Catalog> = {
  en: makeCatalog(),
};

export const catalogFor = (locale: SupportedLocale): Catalog => catalogs[locale];

/** Locale signals in precedence order. Each is optional and untrusted. */
export interface LocaleSignals {
  readonly explicitSetting?: string | undefined;
  readonly urlParam?: string | undefined;
  readonly previousUrlLocale?: string | undefined;
  readonly browserLanguage?: string | undefined;
}

const asSupported = (candidate: string | undefined): SupportedLocale | undefined => {
  if (candidate === undefined) return undefined;
  const primary = candidate.toLowerCase().split("-")[0];
  return SUPPORTED_LOCALES.find((locale) => locale === primary);
};

/**
 * Resolve the active locale: explicit setting, then URL parameter, then the
 * previous URL's locale, then the browser language, then the default. An
 * unsupported value at any level falls through to the next.
 */
export const selectLocale = (signals: LocaleSignals = {}): SupportedLocale =>
  asSupported(signals.explicitSetting) ??
  asSupported(signals.urlParam) ??
  asSupported(signals.previousUrlLocale) ??
  asSupported(signals.browserLanguage) ??
  DEFAULT_LOCALE;
