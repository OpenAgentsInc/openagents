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
    id: "khala",
    title: "Khala UI",
    module: "@effect-native/khala-ui + @effect-native/render-canvas",
    purpose:
      "The complete non-audio Khala visual-effects catalog: foundations, motion, choreography, frames, text, illumination, and animated backgrounds.",
    exports: [
      "12 frame motifs",
      "31 easing curves",
      "Motion and sequence drivers",
      "Text sequence and decipher",
      "HTML and SVG illumination",
      "4 Canvas backgrounds",
    ],
    contract: [
      "30 of 30 visual capabilities mounted",
      "SSR-stable output; effects start after mount",
    ],
  },
  {
    id: "core",
    title: "Effect Native core",
    module: "@effect-native/core",
    purpose:
      "Renderer-neutral layout, controls, overlays, data display, composer, feedback, and public-page views.",
    exports: ["Stack", "Text", "Button", "Card", "Composer", "Transcript", "Table", "Hero"],
    contract: ["Typed view catalog", "No renderer-specific markup"],
  },
  {
    id: "tokens",
    title: "Tokens",
    module: "@effect-native/tokens",
    purpose: "Canonical semantic theme and bounded spacing, type, radius, and control lattices.",
    exports: ["khalaTheme", "colorTokens", "spacingTokens", "radiusTokens", "typeScaleTokens"],
    contract: ["One semantic token authority", "Renderer projections preserve roles"],
  },
  {
    id: "render-dom",
    title: "DOM renderer",
    module: "@effect-native/render-dom",
    purpose: "Lowers the typed Effect Native catalog to accessible DOM output.",
    exports: ["makeDomRenderer", "viewStructure", "serializeDomStructure"],
    contract: ["Core never imports a renderer", "Closed icon registry"],
  },
  {
    id: "render-rn",
    title: "React Native renderer",
    module: "@effect-native/render-rn",
    purpose: "Lowers the same typed catalog to native platform components and interactions.",
    exports: ["makeReactNativeRenderer", "EffectNativeSurface"],
    contract: ["Same core view contract", "Platform behavior stays renderer-owned"],
  },
  {
    id: "training",
    title: "Training grammar",
    module: "oa-training-run / @openagentsinc/three-effect",
    purpose: "Training-run visual grammar references for replay and verification.",
    exports: [
      "Run field",
      "Contributor node",
      "Replay pair",
      "Verification gate",
      "Receipt burst",
      "Proof drawer",
    ],
    contract: ["three-effect owns visuals", "No app-local replay renderer"],
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

function FamilyDetail({ family }: Readonly<{ family: ComponentFamily }>) {
  return (
    <section className="grid gap-5" data-component-family-detail={family.id}>
      <div className="grid gap-2">
        <p className={eyebrowClass}>{family.module}</p>
        <h2 className="m-0 text-balance text-3xl font-semibold tracking-tight text-white sm:text-4xl">
          {family.title}
        </h2>
        <p className="m-0 max-w-[78ch] text-pretty text-base/7 text-khala-text-muted">
          {family.purpose}
        </p>
      </div>
      <div className="grid gap-4 md:grid-cols-2">
        <div className={panelClass}>
          <p className={eyebrowClass}>Catalog anchors</p>
          <ul className="grid gap-2 font-mono text-base text-white sm:text-sm" role="list">
            {family.exports.map((exportName) => (
              <li key={exportName}>{exportName}</li>
            ))}
          </ul>
        </div>
        <div className={panelClass}>
          <p className={eyebrowClass}>Contract</p>
          <ul className="grid gap-2 text-base/7 text-khala-text-muted sm:text-sm/6" role="list">
            {family.contract.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}

export function ComponentsPage({ selectedFamily }: Readonly<{ selectedFamily?: string }>) {
  const family = selectedFamily === undefined ? undefined : familyById.get(selectedFamily);

  return (
    <main className="min-h-dvh bg-black text-white" data-route="components">
      <div className="mx-auto grid w-full max-w-7xl gap-8 px-4 py-8 font-mono sm:px-6 lg:px-8">
        <header className="grid gap-3">
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
            The active Effect Native component, token, and renderer boundaries used by OpenAgents
            surfaces.
          </p>
        </header>
        {selectedFamily === "khala" ? (
          <KhalaComponentsWorkbench />
        ) : family === undefined ? (
          <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            {families.map((item) => (
              <FamilyCard family={item} key={item.id} />
            ))}
          </section>
        ) : (
          <FamilyDetail family={family} />
        )}
      </div>
    </main>
  );
}
import { KhalaComponentsWorkbench } from "./-components-khala-page";
