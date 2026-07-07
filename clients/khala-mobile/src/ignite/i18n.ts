// Minimal i18n stub for the ported Ignite component kit. The Ignite
// boilerplate wires every string through i18next (`@/i18n` + `@/i18n/translate`);
// khala-mobile has no i18next dependency, so this stub satisfies the exact
// surface the ported components import (`isRTL`, `TxKeyPath`, `translate`) while
// leaving translation as a pass-through. Real localization is out of scope for
// this port.
export const isRTL = false

export type TxKeyPath = string

export type TxOptions = Record<string, unknown>

export const translate = (key: string, _opts?: TxOptions | unknown): string => key
