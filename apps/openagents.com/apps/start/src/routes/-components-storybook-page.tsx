import { type IntentReporter, type Theme } from "@effect-native/core";
import { allStories, defaultStorybook, type Story, type StoryGroup } from "@effect-native/gallery";
import { makeDomRenderer } from "@effect-native/render-dom";
import { useEffectNativeScopedEffect } from "@effect-native/render-dom/react";
import { khalaTheme } from "@effect-native/tokens";
import { Effect, Stream } from "effect";
import { type CSSProperties, type ReactElement, useRef } from "react";

const noopReport: IntentReporter = () => Effect.void;

type ThemeCssVariables = CSSProperties & Readonly<Record<`--en-${string}`, string | number>>;

const px = (value: string | number): string => (typeof value === "number" ? `${value}px` : value);

const themeCssVariables = (theme: Theme): ThemeCssVariables => {
  const variables: Record<string, string | number> = {
    backgroundColor: theme.color.background,
    color: theme.color.textPrimary,
  };

  for (const [key, value] of Object.entries(theme.color)) variables[`--en-color-${key}`] = value;
  for (const [key, value] of Object.entries(theme.spacing))
    variables[`--en-spacing-${key}`] = px(value);
  for (const [key, value] of Object.entries(theme.radius))
    variables[`--en-radius-${key}`] = px(value);
  for (const [key, value] of Object.entries(theme.dimension))
    variables[`--en-dimension-${key}`] = px(value);
  for (const [key, value] of Object.entries(theme.typeScale)) {
    variables[`--en-type-${key}-fontSize`] = px(value.fontSize);
    variables[`--en-type-${key}-lineHeight`] = px(value.lineHeight);
    variables[`--en-type-${key}-fontWeight`] = value.fontWeight;
  }
  for (const [key, value] of Object.entries(theme.control)) {
    variables[`--en-control-${key}-height`] = px(value.height);
    variables[`--en-control-${key}-gutter`] = px(value.gutter);
    variables[`--en-control-${key}-radius`] = px(value.radius);
    variables[`--en-control-${key}-font-size`] = px(value.fontSize);
    variables[`--en-control-${key}-icon`] = px(value.icon);
  }

  variables["--en-motion-fast"] = `${theme.motion.durationFastMs}ms`;
  variables["--en-motion-enter"] = `${theme.motion.durationEnterMs}ms`;
  variables["--en-motion-exit"] = `${theme.motion.durationExitMs}ms`;
  variables["--en-motion-loop"] = `${theme.motion.durationLoopMs}ms`;
  variables["--en-ease-basic"] = theme.motion.easeBasic;
  variables["--en-ease-enter"] = theme.motion.easeEnter;
  variables["--en-ease-exit"] = theme.motion.easeExit;
  variables["--en-ease-exit-snappy"] = theme.motion.easeExitSnappy;
  variables["--en-ease-move"] = theme.motion.easeMove;

  return variables as ThemeCssVariables;
};

const storyValue = (value: unknown): string => {
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
};

function StoryPreview({ story }: Readonly<{ story: Story }>): ReactElement {
  const canvasRef = useRef<HTMLDivElement>(null);

  useEffectNativeScopedEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return Effect.void;
    return makeDomRenderer({ theme: story.theme, viewport: story.viewport }).mount(
      canvas,
      Stream.make(story.view),
      noopReport,
    );
  }, [story]);

  return (
    <article
      className="min-w-0 overflow-hidden border border-khala-border/70 bg-khala-surface"
      data-storybook-story={story.id}
    >
      <div
        className="effect-native-story-canvas relative isolate min-h-48 transform-gpu overflow-auto p-5"
        data-effect-native-surface="dom"
        ref={canvasRef}
        style={themeCssVariables(story.theme)}
      >
        <span className="text-xs text-khala-text-faint">Mounting {story.component} preview…</span>
      </div>
      <div className="grid gap-3 border-t border-khala-border/70 bg-black p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="grid gap-1">
            <h3 className="m-0 text-sm font-semibold text-white">{story.title}</h3>
            <p className="m-0 max-w-[64ch] text-xs/5 text-khala-text-muted">{story.description}</p>
          </div>
          <code className="text-[11px] text-khala-energy-cyan">{story.id}</code>
        </div>
        {story.controls.length > 0 ? (
          <dl className="m-0 flex flex-wrap gap-1.5" aria-label={`${story.title} controls`}>
            {story.controls.map((control) => (
              <div
                className="flex items-center gap-1.5 border border-khala-border/70 bg-khala-surface px-2 py-1 text-[11px]"
                key={control.id}
              >
                <dt className="text-khala-text-faint">{control.label}</dt>
                <dd className="m-0 text-khala-text">{storyValue(control.value)}</dd>
              </div>
            ))}
          </dl>
        ) : null}
        <details className="text-xs text-khala-text-muted">
          <summary className="khala-focus w-fit cursor-pointer py-1 text-khala-text-faint">
            Inspect typed view
          </summary>
          <pre className="mt-2 max-h-72 overflow-auto border border-khala-border/70 bg-khala-surface p-3 text-[11px]/5 text-khala-text-muted">
            {JSON.stringify(story.view, null, 2)}
          </pre>
        </details>
      </div>
    </article>
  );
}

const trainingComponents = new Set([
  "Meter",
  "StatTile",
  "Table",
  "Transcript",
  "CodeBlock",
  "DiffView",
  "GraphFigure",
  "Timeline",
  "StatusBanner",
]);

const groupsForFamily = (
  family: "core" | "render-dom" | "render-rn" | "training",
): ReadonlyArray<StoryGroup> => {
  if (family === "training") {
    return defaultStorybook.groups.filter((group) => trainingComponents.has(group.component));
  }
  return defaultStorybook.groups;
};

const familyCopy = {
  core: {
    eyebrow: "@effect-native/core · complete catalog",
    title: "79 components. 108 rendered variants.",
    description:
      "Every owned Effect Native component is mounted below from its typed story fixture. Nothing here is a placeholder list.",
  },
  "render-dom": {
    eyebrow: "@effect-native/render-dom · browser projection",
    title: "The complete DOM renderer gallery.",
    description:
      "The same 108 typed stories lowered to accessible DOM—the renderer production web and Electron React surfaces consume.",
  },
  "render-rn": {
    eyebrow: "@effect-native/render-rn · shared fixture contract",
    title: "React Native parity, shown from the shared views.",
    description:
      "These are the exact renderer-neutral fixtures consumed by React Native. The browser projection keeps every component and variant inspectable here.",
  },
  training: {
    eyebrow: "training grammar · operational components",
    title: "Run, proof, replay, and status primitives.",
    description:
      "The component subset used to communicate progress, verification, diffs, receipts, and training-run state.",
  },
} as const;

export function EffectNativeStorybook({
  family,
}: Readonly<{ family: "core" | "render-dom" | "render-rn" | "training" }>): ReactElement {
  const groups = groupsForFamily(family);
  const stories = groups.flatMap((group) => group.stories);
  const copy = familyCopy[family];

  return (
    <div className="grid gap-8" data-storybook-family={family}>
      <header className="grid gap-5 border-y border-khala-border/80 bg-khala-surface-muted px-5 py-6 sm:px-7">
        <div className="flex flex-wrap items-start justify-between gap-5">
          <div className="grid max-w-3xl gap-2">
            <p className="m-0 text-sm text-khala-energy-cyan">{copy.eyebrow}</p>
            <h2 className="m-0 text-balance text-3xl font-semibold tracking-[-0.025em] text-white sm:text-4xl">
              {copy.title}
            </h2>
            <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted sm:text-base/7">
              {copy.description}
            </p>
          </div>
          <dl className="m-0 grid min-w-52 grid-cols-2 gap-px bg-khala-border/80">
            <div className="bg-black p-4">
              <dt className="text-xs text-khala-text-faint">components</dt>
              <dd className="m-0 mt-1 text-2xl text-white">{groups.length}</dd>
            </div>
            <div className="bg-black p-4">
              <dt className="text-xs text-khala-text-faint">stories</dt>
              <dd className="m-0 mt-1 text-2xl text-white">{stories.length}</dd>
            </div>
          </dl>
        </div>
        <nav className="flex gap-2 overflow-x-auto pb-1" aria-label="Component family routes">
          {[
            ["core", "All components"],
            ["tokens", "Tokens"],
            ["render-dom", "DOM"],
            ["render-rn", "React Native"],
            ["training", "Training"],
            ["khala", "Khala effects"],
          ].map(([id, label]) => (
            <a
              className={`khala-focus shrink-0 border px-3 py-2 text-xs ${
                family === id
                  ? "border-khala-energy bg-khala-energy/10 text-white"
                  : "border-khala-border text-khala-text-muted hover:border-khala-border-strong hover:text-white"
              }`}
              href={`/components/${id}`}
              key={id}
            >
              {label}
            </a>
          ))}
        </nav>
      </header>

      <div className="grid items-start gap-8 xl:grid-cols-[14rem_minmax(0,1fr)]">
        <aside className="hidden max-h-[calc(100dvh-2rem)] overflow-auto border border-khala-border/70 bg-khala-surface p-3 xl:sticky xl:top-4 xl:block">
          <p className="m-0 px-2 pb-2 text-xs text-khala-text-faint">Jump to component</p>
          <nav className="grid" aria-label="Storybook components">
            {groups.map((group) => (
              <a
                className="khala-focus flex items-center justify-between gap-3 px-2 py-1.5 text-xs text-khala-text-muted hover:bg-khala-surface-raised hover:text-white"
                href={`#component-${group.component.toLowerCase()}`}
                key={group.component}
              >
                <span>{group.title}</span>
                <span className="tabular-nums text-khala-text-faint">{group.stories.length}</span>
              </a>
            ))}
          </nav>
        </aside>

        <div className="grid min-w-0 gap-12">
          {groups.map((group) => (
            <section
              className="grid scroll-mt-6 gap-4"
              data-storybook-component={group.component}
              id={`component-${group.component.toLowerCase()}`}
              key={group.component}
            >
              <header className="flex flex-wrap items-end justify-between gap-3 border-b border-khala-border/70 pb-3">
                <div className="grid gap-1">
                  <h2 className="m-0 text-xl font-semibold text-white">{group.title}</h2>
                  <p className="m-0 text-xs text-khala-text-faint">
                    {group.stories.length} {group.stories.length === 1 ? "story" : "variants"}
                  </p>
                </div>
                <a
                  className="khala-focus text-xs text-khala-text-faint hover:text-white"
                  href="#top"
                >
                  Back to top
                </a>
              </header>
              <div className="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(min(100%,22rem),1fr))]">
                {group.stories.map((story) => (
                  <StoryPreview key={story.id} story={story} />
                ))}
              </div>
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}

export function TokenStorybook(): ReactElement {
  const colors = Object.entries(khalaTheme.color);
  const spacing = Object.entries(khalaTheme.spacing);
  const radii = Object.entries(khalaTheme.radius);
  const type = Object.entries(khalaTheme.typeScale);

  return (
    <div className="grid gap-10" data-storybook-family="tokens">
      <header className="grid gap-2 border-y border-khala-border/80 bg-khala-surface-muted px-5 py-6 sm:px-7">
        <p className="m-0 text-sm text-khala-energy-cyan">
          @effect-native/tokens · visual reference
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

export const completeStoryCount = allStories().length;
