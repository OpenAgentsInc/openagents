import { khalaTheme } from "@openagentsinc/design-tokens";
import { type ReactElement } from "react";

/**
 * `/components/tokens` — the design-token visual reference.
 *
 * Lifted out of the former `-components-storybook-page.tsx` when Effect
 * Native was removed (#9325). That module also held an Effect Native
 * component storybook, which had no subject left once the framework was
 * deleted; this half survives because its subject is the token vocabulary,
 * which now lives in `@openagentsinc/design-tokens`.
 */
export function TokenStorybook(): ReactElement {
  const colors = Object.entries(khalaTheme.color);
  const spacing = Object.entries(khalaTheme.spacing);
  const radii = Object.entries(khalaTheme.radius);
  const type = Object.entries(khalaTheme.typeScale);

  return (
    <div className="grid gap-10" data-storybook-family="tokens">
      <header className="grid gap-2 border-y border-khala-border/80 bg-khala-surface-muted px-5 py-6 sm:px-7">
        <p className="m-0 text-sm text-khala-energy-cyan">
          @openagentsinc/design-tokens · visual reference
        </p>
        <h2 className="m-0 text-3xl font-semibold text-white sm:text-4xl">
          Theme values you can actually inspect.
        </h2>
        <p className="m-0 max-w-[70ch] text-sm/6 text-khala-text-muted">
          Color roles, type, spacing, and radii rendered at their real values—not an export-name
          inventory.
        </p>
      </header>
      <section className="grid gap-4" aria-labelledby="token-colors">
        <h2 className="m-0 text-xl text-white" id="token-colors">
          Color roles
        </h2>
        <div className="grid gap-px bg-khala-border/60 [grid-template-columns:repeat(auto-fit,minmax(12rem,1fr))]">
          {colors.map(([name, value]) => (
            <article className="grid min-h-32 content-between gap-4 bg-black p-4" key={name}>
              <div className="h-14 border border-white/10" style={{ backgroundColor: value }} />
              <div className="grid gap-1 text-xs">
                <strong className="font-medium text-white">{name}</strong>
                <code className="text-khala-text-faint">{value}</code>
              </div>
            </article>
          ))}
        </div>
      </section>
      <section className="grid gap-4" aria-labelledby="token-type">
        <h2 className="m-0 text-xl text-white" id="token-type">
          Type scale
        </h2>
        <div className="grid gap-px bg-khala-border/60">
          {type.map(([name, value]) => (
            <div
              className="flex flex-wrap items-baseline justify-between gap-4 bg-black p-4"
              key={name}
            >
              <span
                style={{
                  fontSize: value.fontSize,
                  fontWeight: value.fontWeight,
                  lineHeight: `${value.lineHeight}px`,
                }}
              >
                {name} · OpenAgents signal
              </span>
              <code className="text-xs text-khala-text-faint">
                {value.fontSize}/{value.lineHeight} · {value.fontWeight}
              </code>
            </div>
          ))}
        </div>
      </section>
      <section className="grid gap-6 md:grid-cols-2">
        <div className="grid gap-4">
          <h2 className="m-0 text-xl text-white">Spacing</h2>
          <div className="grid gap-3 border border-khala-border/70 bg-black p-4">
            {spacing.map(([name, value]) => (
              <div className="grid grid-cols-[3rem_1fr_3rem] items-center gap-3 text-xs" key={name}>
                <code className="text-khala-text-faint">{name}</code>
                <div
                  className="h-2 bg-khala-energy-cyan"
                  style={{ width: Math.max(1, Number(value)) }}
                />
                <span className="text-right text-khala-text-muted">{value}px</span>
              </div>
            ))}
          </div>
        </div>
        <div className="grid content-start gap-4">
          <h2 className="m-0 text-xl text-white">Radius</h2>
          <div className="grid grid-cols-2 gap-3 border border-khala-border/70 bg-black p-4">
            {radii.map(([name, value]) => (
              <div className="grid gap-2 text-xs" key={name}>
                <div
                  className="h-20 border border-khala-energy bg-khala-energy/10"
                  style={{ borderRadius: value }}
                />
                <span className="text-khala-text-muted">
                  {name} · {value}px
                </span>
              </div>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}
