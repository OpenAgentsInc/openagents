// Typed message-catalog scaffolding.
//
// - `MessageKey` is derived from the English source catalog, so a removed or
//   renamed key fails typecheck at every call site.
// - A locale ships a `LocaleOverride` — any subset of keys — and
//   `makeCatalog` back-fills the rest from English at load, so a partial
//   locale can never render a blank string.
// - `render` requires the typed parameters of a parameterised message and
//   forbids parameters on a static one.

import { en } from "./en.js";

export type SourceCatalog = typeof en;

export type MessageKey = keyof SourceCatalog;

/** The parameter object of a message key, or `void` for static messages. */
export type MessageParamsOf<K extends MessageKey> = SourceCatalog[K] extends (
  params: infer P,
) => string
  ? P
  : void;

/** A full catalog: every key present, same parameter shape as the source. */
export type Catalog = {
  readonly [K in MessageKey]: SourceCatalog[K] extends (params: infer P) => string
    ? (params: P) => string
    : string;
};

/** A locale's translations: any subset of the source key set. */
export type LocaleOverride = { readonly [K in MessageKey]?: Catalog[K] };

/**
 * Build a complete catalog from a (possibly partial) locale. Missing keys
 * fall back to the English source string.
 */
export const makeCatalog = (override: LocaleOverride = {}): Catalog => {
  const base: Catalog = en;
  return { ...base, ...override };
};

/**
 * Render one message. Parameterised keys require their typed parameters;
 * static keys take none.
 */
export const render = <K extends MessageKey>(
  catalog: Catalog,
  key: K,
  ...params: MessageParamsOf<K> extends void ? [] : [MessageParamsOf<K>]
): string => {
  const message: string | ((p: never) => string) = catalog[key];
  return typeof message === "function" ? message(params[0] as never) : message;
};
