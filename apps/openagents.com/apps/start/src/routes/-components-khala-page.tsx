import { Frame, Stack, Text, type IntentReporter } from "@effect-native/core";
import { khalaUiEffectStories, khalaUiFinalParityReceipt } from "@effect-native/gallery/khala";
import {
  khalaEasingNames,
  khalaSvgIlluminationGradientId,
  makeKhalaDomIlluminator,
  makeKhalaTextDecipherFrames,
  makeKhalaTextSequenceFrames,
  planKhalaChoreography,
  resolveKhalaMotionKeyframes,
  runKhalaDomMotion,
  runKhalaDomTextEffect,
  sampleKhalaEasing,
  type KhalaMotionPreset,
} from "@effect-native/khala-ui";
import {
  makeKhalaCanvasBackground,
  type KhalaCanvasBackground,
} from "@effect-native/render-canvas";
import { renderReactDomView, useEffectNativeScopedEffect } from "@effect-native/render-dom/react";
import {
  khalaMotifIds,
  khalaTheme,
  resolveKhalaSeparatorPaint,
  resolveKhalaStepsPaint,
  resolveKhalaStripPaint,
  type KhalaLinearPaint,
  type KhalaMotifId,
} from "@effect-native/tokens";
import { Effect } from "effect";
import { useRef, useState, type CSSProperties, type ReactElement } from "react";

const noopReport: IntentReporter = () => Effect.void;

type EffectNativeCssVariables = CSSProperties & Readonly<Record<`--en-${string}`, string | number>>;

const effectNativeCssVariables = {
  backgroundColor: "#000000",
  "--en-color-accent": khalaTheme.color.accent,
  "--en-color-borderStrong": khalaTheme.color.borderStrong,
  "--en-color-borderSubtle": khalaTheme.color.borderSubtle,
  "--en-color-focus": khalaTheme.color.focus,
  "--en-color-textPrimary": khalaTheme.color.textPrimary,
  "--en-color-textMuted": khalaTheme.color.textMuted,
  "--en-spacing-2": `${khalaTheme.spacing["2"]}px`,
  "--en-spacing-3": `${khalaTheme.spacing["3"]}px`,
  "--en-spacing-4": `${khalaTheme.spacing["4"]}px`,
  "--en-type-label-fontSize": `${khalaTheme.typeScale.label.fontSize}px`,
  "--en-type-label-lineHeight": `${khalaTheme.typeScale.label.lineHeight}px`,
} satisfies EffectNativeCssVariables;

const sectionLinks = [
  ["foundation", "Foundation"],
  ["motion", "Motion"],
  ["choreography", "Choreography"],
  ["frames", "Frames"],
  ["text", "Text"],
  ["illumination", "Illumination"],
  ["backgrounds", "Backgrounds"],
] as const;

const roleColor = (role: KhalaLinearPaint["stops"][number]["role"]): string => {
  if (role === "transparent") return "transparent";
  if (role === "quiet") return "var(--khala-border)";
  if (role === "structural") return "var(--khala-border-strong)";
  if (role === "focus") return "var(--khala-energy-soft)";
  return "var(--khala-energy-cyan)";
};

const paintCss = (paint: KhalaLinearPaint): string => {
  const stops = paint.stops
    .map((stop) => `${roleColor(stop.role)} ${Math.round(stop.offset * 100)}%`)
    .join(", ");
  const angle = paint.direction === "horizontal" ? "90deg" : "180deg";
  return `${paint.repeating ? "repeating-" : ""}linear-gradient(${angle}, ${stops})`;
};

function SectionHeading({
  id,
  title,
  description,
  count,
}: Readonly<{
  id: string;
  title: string;
  description: string;
  count: number;
}>) {
  return (
    <header className="grid gap-2 border-b border-khala-border/80 pb-4" id={id}>
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <h2 className="m-0 text-2xl font-semibold text-white sm:text-3xl">{title}</h2>
        <span className="font-mono text-sm tabular-nums text-khala-energy-cyan">
          {count} {count === 1 ? "capability" : "capabilities"}
        </span>
      </div>
      <p className="m-0 max-w-[72ch] text-pretty text-sm/6 text-khala-text-muted">{description}</p>
    </header>
  );
}

function CapabilityLabel({ id, name }: Readonly<{ id: string; name: string }>) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2 border-b border-khala-border/60 pb-3">
      <h3 className="m-0 text-base font-medium text-white">{name}</h3>
      <code className="text-xs text-khala-text-faint">{id}</code>
    </div>
  );
}

function FoundationSection() {
  const steps = resolveKhalaStepsPaint(8);
  const strip = resolveKhalaStripPaint(["quiet", "signal", "structural", "focus"]);
  const separators = [
    ["start", resolveKhalaSeparatorPaint("start")],
    ["end", resolveKhalaSeparatorPaint("end")],
    ["both", resolveKhalaSeparatorPaint("both")],
  ] as const;
  const swatches = [
    ["void", khalaTheme.color.background],
    ["surface", khalaTheme.color.surface],
    ["structural", khalaTheme.color.borderStrong],
    ["signal", khalaTheme.color.accent],
    ["focus", khalaTheme.color.focus],
  ] as const;

  return (
    <section
      className="grid scroll-mt-20 gap-6"
      aria-labelledby="foundation"
      data-khala-family="foundation"
    >
      <SectionHeading
        id="foundation"
        title="Foundation"
        count={4}
        description="One typed theme and three bounded paint grammars. Forced colors resolve to structure instead of disappearing."
      />
      <div className="grid gap-px overflow-hidden border border-khala-border/80 bg-khala-border/80 lg:grid-cols-2">
        <article
          className="grid gap-4 bg-khala-surface p-5"
          data-khala-capability="foundation.theme"
        >
          <CapabilityLabel id="foundation.theme" name="Theme roles" />
          <div className="flex min-h-32 flex-wrap items-end gap-3">
            {swatches.map(([name, color], index) => (
              <div className="grid min-w-20 flex-1 gap-2" key={name}>
                <div
                  className="border border-white/10"
                  style={{ background: color, height: `${44 + index * 12}px` }}
                />
                <span className="font-mono text-xs text-khala-text-muted">{name}</span>
              </div>
            ))}
          </div>
        </article>
        <article
          className="grid gap-4 bg-khala-surface p-5"
          data-khala-capability="foundation.steps"
        >
          <CapabilityLabel id="foundation.steps" name="Stepped signal" />
          <div
            className="h-32 border-y border-khala-border/70"
            style={{ background: paintCss(steps) }}
          />
        </article>
        <article
          className="grid gap-4 bg-khala-surface p-5"
          data-khala-capability="foundation.strip"
        >
          <CapabilityLabel id="foundation.strip" name="Repeating strip" />
          <div className="h-32" style={{ background: paintCss(strip) }} />
        </article>
        <article
          className="grid gap-4 bg-khala-surface p-5"
          data-khala-capability="foundation.separator"
        >
          <CapabilityLabel id="foundation.separator" name="Signal separators" />
          <div className="grid content-center gap-6 py-3">
            {separators.map(([name, paint]) => (
              <div className="grid grid-cols-[4rem_1fr] items-center gap-3" key={name}>
                <span className="font-mono text-xs text-khala-text-muted">{name}</span>
                <div className="h-1" style={{ background: paintCss(paint) }} />
              </div>
            ))}
          </div>
        </article>
      </div>
    </section>
  );
}

const easingPoints = (name: (typeof khalaEasingNames)[number]): string =>
  Array.from({ length: 25 }, (_, index) => {
    const x = index / 24;
    const y = sampleKhalaEasing(name, x);
    return `${Math.round(x * 96)},${Math.round(42 - y * 36)}`;
  }).join(" ");

const motionPresets: ReadonlyArray<readonly [string, KhalaMotionPreset]> = [
  ["Property", { _tag: "Property", property: "x", from: -18, to: 18 }],
  ["Fade", { _tag: "Fade" }],
  ["Flicker", { _tag: "Flicker" }],
  ["Stroke draw", { _tag: "StrokeDraw", length: 120 }],
];

function MotionReplay({ preset, label }: Readonly<{ preset: KhalaMotionPreset; label: string }>) {
  const target = useRef<HTMLDivElement>(null);
  const [replay, setReplay] = useState(0);

  useEffectNativeScopedEffect(() => {
    const element = target.current;
    if (element === null) return Effect.void;
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    return runKhalaDomMotion(element, preset, "enter", {
      durationMillis: 700,
      reducedMotion,
    });
  }, [preset, replay]);

  return (
    <div className="grid gap-3 border-t border-khala-border/60 pt-3">
      <div className="grid h-20 place-items-center overflow-hidden bg-black/60">
        <div className="h-8 w-24 border border-khala-energy-cyan bg-khala-energy/15" ref={target} />
      </div>
      <button
        className="khala-focus min-h-11 w-fit border border-khala-border-strong bg-black px-4 font-mono text-sm text-white hover:bg-khala-surface-raised"
        onClick={() => setReplay((value) => value + 1)}
        type="button"
      >
        Replay {label}
      </button>
    </div>
  );
}

function MotionSection() {
  const sequence = planKhalaChoreography({
    manager: "sequence",
    target: "entered",
    children: [
      { id: "signal", enterMillis: 120, exitMillis: 80 },
      { id: "structure", enterMillis: 180, exitMillis: 100 },
      { id: "content", enterMillis: 220, exitMillis: 120 },
    ],
  });

  return (
    <>
      <section
        className="grid scroll-mt-20 gap-6"
        aria-labelledby="motion"
        data-khala-family="motion"
      >
        <SectionHeading
          id="motion"
          title="Motion"
          count={6}
          description="Pure sampled curves and Effect-owned drivers. Replays are deliberate; content is already visible before motion begins."
        />
        <article
          className="grid gap-4 border border-khala-border/80 bg-khala-surface p-5"
          data-khala-capability="motion.easing"
        >
          <CapabilityLabel id="motion.easing" name="31 easing curves" />
          <div className="grid gap-px bg-khala-border/60 sm:grid-cols-2 lg:grid-cols-4">
            {khalaEasingNames.map((name) => (
              <figure className="m-0 grid gap-2 bg-black p-3" key={name}>
                <svg aria-hidden="true" className="h-12 w-full" viewBox="0 0 96 48">
                  <polyline
                    fill="none"
                    points={easingPoints(name)}
                    stroke="var(--khala-energy-cyan)"
                    strokeWidth="1.5"
                  />
                </svg>
                <figcaption className="truncate font-mono text-xs text-khala-text-muted">
                  {name}
                </figcaption>
              </figure>
            ))}
          </div>
        </article>
        <div className="grid gap-px bg-khala-border/60 md:grid-cols-2">
          <article
            className="grid gap-4 bg-khala-surface p-5"
            data-khala-capability="motion.css-properties"
          >
            <CapabilityLabel id="motion.css-properties" name="Typed CSS properties" />
            <div className="flex flex-wrap gap-2 font-mono text-sm text-khala-text-muted">
              {["opacity", "translate", "scale", "rotate", "skew", "stroke"].map((item) => (
                <span className="border border-khala-border px-3 py-2" key={item}>
                  {item}
                </span>
              ))}
            </div>
          </article>
          <article
            className="grid gap-4 bg-khala-surface p-5"
            data-khala-capability="motion.driver"
          >
            <CapabilityLabel id="motion.driver" name="Scoped driver" />
            <MotionReplay
              label="driver"
              preset={{ _tag: "Property", property: "scale", from: 0.76, to: 1 }}
            />
          </article>
          <article
            className="grid gap-4 bg-khala-surface p-5"
            data-khala-capability="motion.element"
          >
            <CapabilityLabel id="motion.element" name="Element states" />
            <div className="grid grid-cols-4 gap-2 text-center font-mono text-xs">
              {["exited", "entering", "entered", "exiting"].map((state, index) => (
                <div className="grid gap-2" key={state}>
                  <div
                    className="h-12 border border-khala-border"
                    style={{ opacity: 0.25 + index * 0.25 }}
                  />
                  <span className="text-khala-text-muted">{state}</span>
                </div>
              ))}
            </div>
          </article>
          <article
            className="grid gap-4 bg-khala-surface p-5"
            data-khala-capability="motion.presets"
          >
            <CapabilityLabel id="motion.presets" name="Motion presets" />
            <div className="grid grid-cols-2 gap-2 font-mono text-xs text-khala-text-muted">
              {motionPresets.map(([label, preset]) => (
                <div className="border border-khala-border p-3" key={label}>
                  <span className="text-white">{label}</span>
                  <span className="mt-1 block">
                    {resolveKhalaMotionKeyframes(preset, "enter").length} frames
                  </span>
                </div>
              ))}
            </div>
          </article>
          <article
            className="grid gap-4 bg-khala-surface p-5 md:col-span-2"
            data-khala-capability="frame.assembly"
          >
            <CapabilityLabel id="frame.assembly" name="Frame assembly" />
            <div className="grid gap-3 sm:grid-cols-3">
              {(["background", "line", "deco"] as const).map((phase) => (
                <div className="grid gap-2 border border-khala-border p-3" key={phase}>
                  <span className="font-mono text-sm text-white">{phase}</span>
                  <div className="h-1 bg-khala-energy-cyan" />
                  <span className="font-mono text-xs text-khala-text-faint">
                    {resolveKhalaMotionKeyframes({ _tag: "FrameAssembly", phase }, "enter")
                      .map((frame) => frame.offset)
                      .join(" → ")}
                  </span>
                </div>
              ))}
            </div>
          </article>
        </div>
      </section>

      <section
        className="grid scroll-mt-20 gap-6"
        aria-labelledby="choreography"
        data-khala-family="choreography"
      >
        <SectionHeading
          id="choreography"
          title="Choreography"
          count={1}
          description="One four-state graph coordinates parallel, sequence, reverse, stagger, switch, merge, and combine plans."
        />
        <article
          className="grid gap-5 border border-khala-border/80 bg-khala-surface p-5"
          data-khala-capability="choreography.animator"
        >
          <CapabilityLabel id="choreography.animator" name="Sequence plan" />
          <div className="grid gap-3">
            {sequence.map((step) => (
              <div className="grid grid-cols-[6rem_1fr] items-center gap-3" key={step.id}>
                <span className="font-mono text-xs text-khala-text-muted">{step.id}</span>
                <div className="relative h-5 bg-black">
                  <div
                    className="absolute inset-y-0 bg-khala-energy-cyan"
                    style={{
                      left: `${step.offsetMillis / 5}px`,
                      width: `${Math.max(16, step.durationMillis / 3)}px`,
                    }}
                  />
                </div>
              </div>
            ))}
          </div>
          <div className="flex flex-wrap gap-2 font-mono text-xs text-khala-text-muted">
            {[
              "parallel",
              "sequence",
              "reverse",
              "stagger",
              "reverse stagger",
              "switch",
              "merge",
              "combine",
            ].map((manager) => (
              <span className="border border-khala-border px-2 py-1" key={manager}>
                {manager}
              </span>
            ))}
          </div>
        </article>
      </section>
    </>
  );
}

const upstreamFrameRows = [
  ["frame.underline", "edge-underline"],
  ["frame.lines", "corner-line-array"],
  ["frame.corners", "corner-brackets"],
  ["frame.octagon", "octagonal-surface"],
  ["frame.nero", "corner-chevron"],
  ["frame.nefrex", "split-corner"],
  ["frame.kranox", "asymmetric-cut"],
  ["frame.header", "header-rail"],
  ["frame.circle", "radial-dial"],
] as const satisfies ReadonlyArray<readonly [string, KhalaMotifId]>;

function FramePreview({
  capability,
  motif,
}: Readonly<{ capability?: string; motif: KhalaMotifId }>) {
  const view = Frame(
    {
      key: `khala-workbench-${motif}`,
      khala: {
        id: `workbench-${motif}`,
        motif,
        width: 320,
        height: 132,
        density: "comfortable",
      },
    },
    [
      Stack({ key: `${motif}-content`, direction: "column", gap: "2", padding: "3" }, [
        Text({ key: `${motif}-name`, content: motif, variant: "label", color: "textPrimary" }),
      ]),
    ],
  );

  return (
    <article
      className="min-w-0 bg-black p-3"
      data-khala-capability={capability}
      data-khala-motif={motif}
    >
      {renderReactDomView(view, { report: noopReport, theme: khalaTheme })}
    </article>
  );
}

function FramesSection() {
  return (
    <section
      className="grid scroll-mt-20 gap-6"
      aria-labelledby="frames"
      data-khala-family="frames"
    >
      <SectionHeading
        id="frames"
        title="Frames"
        count={11}
        description="Twelve owned motifs resolve through one bounded scene algebra. Decoration remains inert and content stays outside clipping."
      />
      <div className="grid gap-px overflow-hidden border border-khala-border/80 bg-khala-border/80 sm:grid-cols-2 xl:grid-cols-3">
        {upstreamFrameRows.map(([capability, motif]) => (
          <FramePreview capability={capability} key={motif} motif={motif} />
        ))}
        <article className="grid gap-3 bg-black p-3" data-khala-capability="frame.generic">
          {(["cut-corner-surface", "header-line", "signal-separator"] as const).map((motif) => (
            <FramePreview key={motif} motif={motif} />
          ))}
        </article>
        <article
          className="grid content-center gap-4 bg-black p-5"
          data-khala-capability="frame.clipping"
        >
          <CapabilityLabel id="frame.clipping" name="Decorative clipping" />
          <div className="grid grid-cols-2 gap-3">
            <div
              className="h-20 bg-khala-energy/25"
              style={{
                clipPath:
                  "polygon(8px 0, calc(100% - 8px) 0, 100% 8px, 100% calc(100% - 8px), calc(100% - 8px) 100%, 8px 100%, 0 calc(100% - 8px), 0 8px)",
              }}
            />
            <div
              className="h-20 bg-khala-energy-cyan/20"
              style={{
                clipPath:
                  "polygon(12px 0, 100% 0, 100% calc(100% - 12px), calc(100% - 12px) 100%, 0 100%, 0 12px)",
              }}
            />
          </div>
          <p className="m-0 text-xs/5 text-khala-text-muted">
            Background only. Focus and content are never clipped.
          </p>
        </article>
      </div>
      <p className="m-0 font-mono text-xs text-khala-text-faint">
        Motif authority: {khalaMotifIds.join(" · ")}
      </p>
    </section>
  );
}

function TextEffectPreview({ kind }: Readonly<{ kind: "sequence" | "decipher" }>) {
  const root = useRef<HTMLDivElement>(null);
  const semantic = useRef<HTMLSpanElement>(null);
  const [replay, setReplay] = useState(0);
  const target = kind === "sequence" ? "SIGNAL ACQUIRED" : "KHALA ONLINE";

  useEffectNativeScopedEffect(() => {
    const rootElement = root.current;
    const semanticElement = semantic.current;
    if (rootElement === null || semanticElement === null) return Effect.void;
    const frames =
      kind === "sequence"
        ? makeKhalaTextSequenceFrames(target, { caret: true })
        : makeKhalaTextDecipherFrames(target, 42);
    return runKhalaDomTextEffect(rootElement, semanticElement, frames, {
      durationMillis: 900,
      reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    });
  }, [kind, replay, target]);

  return (
    <div className="grid gap-4">
      <div
        className="relative grid h-24 place-items-center bg-black font-mono text-lg text-khala-energy-cyan"
        ref={root}
      >
        <span ref={semantic}>{target}</span>
      </div>
      <button
        className="khala-focus min-h-11 w-fit border border-khala-border-strong bg-black px-4 font-mono text-sm text-white hover:bg-khala-surface-raised"
        onClick={() => setReplay((value) => value + 1)}
        type="button"
      >
        Replay {kind}
      </button>
    </div>
  );
}

function TextSection() {
  return (
    <section className="grid scroll-mt-20 gap-6" aria-labelledby="text" data-khala-family="text">
      <SectionHeading
        id="text"
        title="Text"
        count={2}
        description="Grapheme-aware visuals run over a temporary inert duplicate. The complete semantic string stays stable."
      />
      <div className="grid gap-px bg-khala-border/80 md:grid-cols-2">
        <article className="grid gap-4 bg-khala-surface p-5" data-khala-capability="text.sequence">
          <CapabilityLabel id="text.sequence" name="Sequence and caret" />
          <TextEffectPreview kind="sequence" />
        </article>
        <article className="grid gap-4 bg-khala-surface p-5" data-khala-capability="text.decipher">
          <CapabilityLabel id="text.decipher" name="Seeded decipher" />
          <TextEffectPreview kind="decipher" />
        </article>
      </div>
    </section>
  );
}

function HtmlIllumination() {
  const root = useRef<HTMLButtonElement>(null);
  const light = useRef<HTMLSpanElement>(null);

  useEffectNativeScopedEffect(() => {
    const rootElement = root.current;
    const lightElement = light.current;
    if (rootElement === null || lightElement === null) return Effect.void;
    return makeKhalaDomIlluminator(rootElement, lightElement, {
      descriptor: { color: "#4fd0ff", radius: 150, intensity: 0.58 },
      reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
      coarsePointer: window.matchMedia("(pointer: coarse)").matches,
    });
  }, []);

  return (
    <button
      className="khala-focus relative grid h-44 w-full place-items-center overflow-hidden border border-khala-border bg-black text-left"
      ref={root}
      type="button"
    >
      <span className="relative z-10 font-mono text-sm text-white">Move pointer or focus</span>
      <span className="absolute inset-0 transition-opacity duration-150" ref={light} />
    </button>
  );
}

function SvgIllumination() {
  const root = useRef<SVGSVGElement>(null);
  const gradient = useRef<SVGRadialGradientElement>(null);
  const stableId = "khala-workbench-svg-light";
  const gradientId = khalaSvgIlluminationGradientId(stableId);

  useEffectNativeScopedEffect(() => {
    const rootElement = root.current;
    const gradientElement = gradient.current;
    if (rootElement === null || gradientElement === null) return Effect.void;
    return makeKhalaDomIlluminator(rootElement, gradientElement, {
      descriptor: { color: "#4fd0ff", radius: 130, intensity: 0.62 },
      mode: "svg",
      stableId,
      reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
      coarsePointer: window.matchMedia("(pointer: coarse)").matches,
    });
  }, [stableId]);

  return (
    <svg
      aria-label="Interactive SVG illumination field"
      className="h-44 w-full border border-khala-border bg-black"
      ref={root}
      role="img"
      tabIndex={0}
      viewBox="0 0 360 176"
    >
      <defs>
        <radialGradient id={gradientId} ref={gradient} gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#4fd0ff" stopOpacity="0.72" />
          <stop offset="1" stopColor="#4fd0ff" stopOpacity="0" />
        </radialGradient>
      </defs>
      <path d="M18 56 H342 M18 88 H342 M18 120 H342" stroke="#17315f" />
      <rect fill={`url(#${gradientId})`} height="176" width="360" />
      <text fill="#f1efe8" fontFamily="monospace" fontSize="13" x="24" y="34">
        SVG / LOCAL COORDINATES
      </text>
    </svg>
  );
}

function IlluminationSection() {
  return (
    <section
      className="grid scroll-mt-20 gap-6"
      aria-labelledby="illumination"
      data-khala-family="illumination"
    >
      <SectionHeading
        id="illumination"
        title="Illumination"
        count={2}
        description="One container-local coordinate driver paints HTML and SVG. Keyboard, coarse pointer, and reduced motion resolve intentionally."
      />
      <div className="grid gap-px bg-khala-border/80 md:grid-cols-2">
        <article
          className="grid gap-4 bg-khala-surface p-5"
          data-khala-capability="illumination.html"
        >
          <CapabilityLabel id="illumination.html" name="HTML radial light" />
          <HtmlIllumination />
        </article>
        <article
          className="grid gap-4 bg-khala-surface p-5"
          data-khala-capability="illumination.svg"
        >
          <CapabilityLabel id="illumination.svg" name="SVG radial light" />
          <SvgIllumination />
        </article>
      </div>
    </section>
  );
}

const backgroundDescriptors: ReadonlyArray<readonly [string, string, KhalaCanvasBackground]> = [
  [
    "background.dots",
    "Dots · box / circle / cross",
    { kind: "dots", shape: "cross", color: "#4fd0ff", spacing: 28, origin: [0.5, 0.5] },
  ],
  [
    "background.grid-lines",
    "GridLines · independent dashes",
    {
      kind: "grid-lines",
      color: "#3a7bff",
      spacing: 42,
      horizontalDash: [5, 13],
      verticalDash: [2, 10],
    },
  ],
  [
    "background.moving-lines",
    "MovingLines · seeded travel",
    { kind: "moving-lines", color: "#4fd0ff", count: 22, direction: "down", glow: 10, seed: 42 },
  ],
  [
    "background.puffs",
    "Puffs · seeded radial fields",
    { kind: "puffs", color: "#3a7bff", count: 12, minRadius: 20, maxRadius: 110, seed: 42 },
  ],
];

function CanvasBackgroundPreview({ descriptor }: Readonly<{ descriptor: KhalaCanvasBackground }>) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffectNativeScopedEffect(() => {
    const element = canvas.current;
    if (element === null) return Effect.void;
    return makeKhalaCanvasBackground(element, descriptor, {
      policy: {
        reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
      },
    });
  }, [descriptor]);

  return <canvas aria-hidden="true" className="h-48 w-full bg-black" ref={canvas} />;
}

function BackgroundsSection() {
  return (
    <section
      className="grid scroll-mt-20 gap-6"
      aria-labelledby="backgrounds"
      data-khala-family="backgrounds"
    >
      <SectionHeading
        id="backgrounds"
        title="Canvas backgrounds"
        count={4}
        description="Four deterministic families share one bounded surface lifecycle. Hidden, offscreen, unfocused, low-power, and reduced-motion hosts suspend."
      />
      <div className="grid gap-px bg-khala-border/80 md:grid-cols-2">
        {backgroundDescriptors.map(([capability, label, descriptor]) => (
          <article
            className="grid gap-4 bg-khala-surface p-5"
            data-khala-capability={capability}
            data-khala-canvas={descriptor.kind}
            key={capability}
          >
            <CapabilityLabel id={capability} name={label} />
            <CanvasBackgroundPreview descriptor={descriptor} />
          </article>
        ))}
      </div>
    </section>
  );
}

function CapabilityRegister() {
  return (
    <details className="border border-khala-border/80 bg-khala-surface">
      <summary className="khala-focus cursor-pointer px-5 py-4 font-mono text-sm text-white">
        Inspect the 30-row parity register
      </summary>
      <div className="grid gap-px border-t border-khala-border/80 bg-khala-border/60 sm:grid-cols-2 lg:grid-cols-3">
        {khalaUiEffectStories.map((story) => (
          <a
            className="khala-focus grid gap-2 bg-black p-4 hover:bg-khala-surface-raised"
            href={`#${story.capabilityId.split(".")[0]}`}
            key={story.capabilityId}
          >
            <code className="text-xs text-khala-energy-cyan">{story.capabilityId}</code>
            <span className="text-xs/5 text-khala-text-muted">{story.variants.join(" · ")}</span>
          </a>
        ))}
      </div>
    </details>
  );
}

export function KhalaComponentsWorkbench(): ReactElement {
  return (
    <div
      className="grid gap-10"
      data-khala-audio="excluded"
      data-khala-capability-count={khalaUiFinalParityReceipt.shippedRows}
      data-khala-workbench="complete"
      style={effectNativeCssVariables}
    >
      <header className="grid gap-5 border-y border-khala-border/80 bg-khala-surface-muted px-5 py-6 sm:px-7">
        <div className="flex flex-wrap items-start justify-between gap-5">
          <div className="grid max-w-3xl gap-3">
            <p className="m-0 font-mono text-sm text-khala-energy-cyan">
              Khala UI / visual systems
            </p>
            <h2 className="m-0 text-balance text-3xl font-semibold tracking-[-0.025em] text-white sm:text-4xl">
              Complete non-audio effects library
            </h2>
            <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted sm:text-base/7">
              Every visual capability from the pinned parity ledger, rendered through the same typed
              contracts used by web, React Native, and Electron.
            </p>
          </div>
          <dl className="m-0 grid min-w-48 grid-cols-2 gap-px bg-khala-border/80 font-mono">
            <div className="bg-black p-4">
              <dt className="text-xs text-khala-text-faint">shipped</dt>
              <dd className="m-0 mt-1 text-2xl text-white">
                {khalaUiFinalParityReceipt.shippedRows}/30
              </dd>
            </div>
            <div className="bg-black p-4">
              <dt className="text-xs text-khala-text-faint">audio</dt>
              <dd className="m-0 mt-1 text-sm text-khala-warning">excluded</dd>
            </div>
          </dl>
        </div>
        <nav aria-label="Khala effect families" className="flex flex-wrap gap-2">
          {sectionLinks.map(([id, label]) => (
            <a
              className="khala-focus border border-khala-border px-3 py-2 font-mono text-xs text-khala-text-muted hover:border-khala-border-strong hover:text-white"
              href={`#${id}`}
              key={id}
            >
              {label}
            </a>
          ))}
        </nav>
      </header>

      <CapabilityRegister />
      <FoundationSection />
      <MotionSection />
      <FramesSection />
      <TextSection />
      <IlluminationSection />
      <BackgroundsSection />

      <footer className="border-t border-khala-border/80 py-6 text-sm/6 text-khala-text-muted">
        Stable semantic output is present before JavaScript. Reduced motion removes continuous work.
        Audio is not part of Khala UI.
      </footer>
    </div>
  );
}
