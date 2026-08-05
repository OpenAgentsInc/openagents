import { TokenStorybook } from "./-components-tokens-page";
import { WorkbenchStorybook } from "./-components-workbench-page";

/**
 * `/components` — the internal design-system workbench.
 *
 * Effect Native was removed in #9325. The four families whose subject was the
 * framework itself — the core catalog storybook, the DOM and
 * React Native renderer galleries, and the Khala visual-effects catalog — had
 * no subject left once the framework was deleted, so they were retired rather
 * than reimplemented. What survives is what was never Effect Native: the
 * shared product workbench components in `@openagentsinc/ui` and the token
 * vocabulary in `@openagentsinc/design-tokens`.
 */
type ComponentFamily = Readonly<{
  id: string;
  title: string;
  module: string;
  purpose: string;
  exports: ReadonlyArray<string>;
  contract: ReadonlyArray<string>;
}>;

const families: ReadonlyArray<ComponentFamily> = [
  {
    id: "workbench",
    title: "Product workbench",
    module: "@openagentsinc/ui/desktop-workbench",
    purpose: "Typed Desktop and web conversation components projected from provider item facts.",
    exports: ["Command execution", "File changes", "Tool calls", "Plans", "Agents", "Approvals"],
    contract: ["Same components on Desktop and web", "Khala is the sole mounted theme"],
  },
  {
    id: "tokens",
    title: "Tokens",
    module: "@openagentsinc/design-tokens",
    purpose: "Canonical semantic theme and bounded spacing, type, radius, and control lattices.",
    exports: ["khalaTheme", "Autopilot donor roles", "colorTokens", "spacingTokens", "radiusTokens", "typeScaleTokens"],
    contract: ["Khala is the sole mounted product theme", "Autopilot grammar resolves through Khala roles"],
  },
];

const familyById = new Map(families.map((family) => [family.id, family]));
const panelClass =
  "grid gap-4 border border-khala-border/80 bg-khala-surface p-5 text-khala-text-muted";
const eyebrowClass = "m-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint";

function FamilyCard({ family }: Readonly<{ family: ComponentFamily }>) {
  return (
    <article className={panelClass} data-component-family={family.id}>
      <div className="grid gap-2">
        <p className={eyebrowClass}>{family.module}</p>
        <h2 className="m-0 text-balance text-2xl font-semibold tracking-tight text-white">
          {family.title}
        </h2>
        <p className="m-0 text-pretty text-base/7 text-khala-text-muted sm:text-sm/6">
          {family.purpose}
        </p>
      </div>
      <a
        className="khala-focus w-fit border border-khala-border bg-khala-surface-raised px-3 py-2 font-mono text-sm text-khala-text"
        href={`/components/${family.id}`}
      >
        Open family
      </a>
    </article>
  );
}

export function ComponentsPage({ selectedFamily }: Readonly<{ selectedFamily?: string }>) {
  const family = selectedFamily === undefined ? undefined : familyById.get(selectedFamily);

  return (
    <main className="min-h-dvh bg-black text-white" data-route="components">
      <div className="mx-auto grid w-full max-w-[100rem] gap-8 px-4 py-8 font-mono sm:px-6 lg:px-8">
        <header className="grid gap-3" id="top">
          <a
            className="khala-focus w-fit border border-khala-border bg-khala-surface-raised px-3 py-2 font-mono text-sm text-khala-text"
            href="/"
          >
            OpenAgents
          </a>
          <p className={eyebrowClass}>Internal - design-system workbench</p>
          <h1 className="m-0 text-balance text-4xl font-semibold tracking-tight text-white sm:text-5xl">
            Component library
          </h1>
          <p className="m-0 max-w-[78ch] text-pretty text-base/7 text-khala-text-muted">
            The active OpenAgents component and token boundaries used by OpenAgents surfaces.
          </p>
        </header>
        {selectedFamily === "workbench" ? (
          <WorkbenchStorybook />
        ) : selectedFamily === "tokens" ? (
          <TokenStorybook />
        ) : family === undefined ? (
          <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            {families.map((item) => (
              <FamilyCard family={item} key={item.id} />
            ))}
          </section>
        ) : null}
      </div>
    </main>
  );
}
