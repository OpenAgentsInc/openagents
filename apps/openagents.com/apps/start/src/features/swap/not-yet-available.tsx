/**
 * The honest placeholder for a swap surface whose owning package has not
 * landed (SWAP-7, #9322). It names the owning issue, states what the surface
 * will do, and renders no control that pretends the capability exists.
 */
export function SwapNotYetAvailable({
  body,
  heading,
  issue,
  marker,
}: Readonly<{
  body: string
  heading: string
  issue: string
  marker: string
}>) {
  return (
    <section
      className="grid gap-2 border border-dashed border-khala-border/80 bg-khala-surface/40 p-6"
      data-swap-not-yet-available={marker}
    >
      <p className="m-0 font-mono text-xs uppercase tracking-wide text-khala-text-faint">
        Not yet available
      </p>
      <h2 className="m-0 text-xl font-semibold text-white">{heading}</h2>
      <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted">
        {body}
      </p>
      <p className="m-0 font-mono text-xs text-khala-text-faint">
        Tracked as {issue}.
      </p>
    </section>
  )
}
