// Shared scene layer used by legacy/interim app-shell pages that still
// render as plain React (currently `-code-page.tsx`'s `/code` landing).
//
// `KhalaInfoPage` and `TassadarInfoPage`, which previously lived here, were
// converted to the Effect Native DOM renderer under EN-4 (#8573) and moved
// to `-khala-effect-native-page.tsx` / `-tassadar-effect-native-page.tsx`.

export function SceneLayer({ pose }: Readonly<{ pose: 'khala' | 'tassadar' }>) {
  const nodeClass =
    pose === 'khala'
      ? 'left-[24%] top-[28%] size-2 opacity-70'
      : 'left-[72%] top-[24%] size-2 opacity-70'

  return (
    <div
      aria-hidden="true"
      className="absolute inset-0 z-0"
      data-persistent-scene="landing-squares"
      data-pose={pose}
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_45%,rgba(58,123,255,0.22),transparent_30%),linear-gradient(180deg,rgba(0,0,0,0.35),#000_82%)]" />
      <div className="absolute inset-0 opacity-35 [background-image:linear-gradient(rgba(58,123,255,0.16)_1px,transparent_1px),linear-gradient(90deg,rgba(58,123,255,0.16)_1px,transparent_1px)] [background-size:4rem_4rem] [mask-image:radial-gradient(circle_at_50%_45%,black,transparent_70%)]" />
      <div className="absolute top-1/2 left-1/2 aspect-square w-[min(76vw,34rem)] -translate-1/2 border border-khala-energy/20 bg-khala-energy/5 khala-glow" />
      <div className="absolute top-1/2 left-1/2 aspect-square w-[min(48vw,22rem)] -translate-1/2 rotate-45 border border-khala-energy-cyan/25 bg-khala-surface/40 khala-glow" />
      <span
        className={`absolute rounded-xs bg-khala-energy-cyan shadow-[0_0_18px_rgba(79,208,255,0.75)] ${nodeClass}`}
      />
      <span className="absolute top-[64%] left-[34%] size-1 rounded-xs bg-khala-energy-cyan opacity-50 shadow-[0_0_18px_rgba(79,208,255,0.75)]" />
      <span className="absolute top-[58%] left-[84%] size-1.5 rounded-xs bg-khala-energy-cyan opacity-45 shadow-[0_0_18px_rgba(79,208,255,0.75)]" />
    </div>
  )
}
