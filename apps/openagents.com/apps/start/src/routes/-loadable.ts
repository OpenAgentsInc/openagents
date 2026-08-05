// Generic fail-soft loader state shared by route data modules.
//
// A surface is either still loading, holding decoded data, or honestly
// unavailable with a reason. There is no fourth state and no "assume ok"
// fallback: an unavailable projection must render an honest empty/degraded
// state, never fabricated content.
//
// This type previously lived in the (now deleted) Electron Desktop download
// data module; it is deliberately product-neutral so the Omega download
// surface and any future consumer can share it.
export type Loadable<T> =
  | { readonly state: 'loading' }
  | { readonly state: 'ok'; readonly data: T }
  | { readonly state: 'unavailable'; readonly detail: string }
