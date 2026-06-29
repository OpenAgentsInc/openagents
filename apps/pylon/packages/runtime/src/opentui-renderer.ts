// Interactive Probe CLI terminal rendering, built on `@opentui/core`.
//
// IMPORTANT: this module must only ever be loaded via the lazy
// `loadProbeOpentuiRenderer()` entry point below, never imported statically
// from the runtime's static module graph (`src/index.ts` -> `src/cli.ts`).
//
// `@opentui/core` resolves platform-native optional dependencies (for example
// `@opentui/core-darwin-x64`) through cross-platform dynamic `import()` calls
// that a bundler cannot statically resolve. If a bundler ever traverses into
// `@opentui/core`, `bun build` of the headless Pylon node fails (#5037).
//
// To keep the headless node bundle-able while preserving full interactive TUI
// rendering for humans, this module never statically imports `@opentui/core`.
// Instead `loadProbeOpentuiRenderer()` resolves it at runtime through a
// computed specifier that the bundler cannot follow, so `@opentui/core` stays
// out of the bundle entirely. The headless node never calls this loader and
// never needs `@opentui/core` installed.

// Type-only import: fully erased at build time, so it never pulls
// `@opentui/core` into the bundle but still gives us precise types.
import type {
  CliRenderer,
  MarkdownRenderable,
  CodeRenderable,
  LineNumberRenderable,
  ScrollBoxRenderable,
  SyntaxStyle,
  TextRenderable,
  BoxRenderable,
  parseColor as ParseColorFn,
} from "@opentui/core";

export interface ProbeRenderer {
  readonly renderer: CliRenderer;
  readonly syntaxStyle: SyntaxStyle;
  readonly session: ScrollBoxRenderable;
  readonly text: MarkdownRenderable;
}

// The runtime shape of `@opentui/core` we depend on. Resolved lazily so the
// bundler never traverses into the package's platform-native dynamic imports.
interface OpentuiCore {
  readonly createCliRenderer: (options: {
    readonly exitOnCtrlC: boolean;
    readonly targetFps: number;
    readonly screenMode: string;
  }) => Promise<CliRenderer>;
  readonly MarkdownRenderable: typeof MarkdownRenderable;
  readonly CodeRenderable: typeof CodeRenderable;
  readonly LineNumberRenderable: typeof LineNumberRenderable;
  readonly ScrollBoxRenderable: typeof ScrollBoxRenderable;
  readonly TextRenderable: typeof TextRenderable;
  readonly BoxRenderable: typeof BoxRenderable;
  readonly SyntaxStyle: typeof SyntaxStyle;
  readonly parseColor: typeof ParseColorFn;
}

async function importOpentuiCore(): Promise<OpentuiCore> {
  // A computed specifier keeps the bundler from statically resolving (and thus
  // traversing into) `@opentui/core`; it resolves normally at runtime.
  const specifier = ["@opentui", "core"].join("/");
  return (await import(specifier)) as unknown as OpentuiCore;
}

/**
 * The opentui-backed rendering surface the interactive Probe CLI needs.
 *
 * `cli.ts` loads this lazily via {@link loadProbeOpentuiRenderer} so the
 * headless runtime never references `@opentui/core` in its static module graph.
 */
export interface ProbeOpentuiRendererModule {
  readonly createProbeRenderer: () => Promise<CliRenderer>;
  readonly createAssistantText: (renderer: CliRenderer) => MarkdownRenderable;
  readonly createCodeWithLineNumbers: (
    renderer: CliRenderer,
    content: string,
    filetype?: string,
  ) => LineNumberRenderable;
  readonly createDefaultSyntaxStyle: () => SyntaxStyle;
  readonly parseColor: typeof ParseColorFn;
  readonly TextRenderable: typeof TextRenderable;
  readonly BoxRenderable: typeof BoxRenderable;
  readonly ScrollBoxRenderable: typeof ScrollBoxRenderable;
}

/**
 * Lazily expose the opentui rendering surface. This is the ONLY supported way
 * for the runtime to reach `@opentui/core`: it keeps the package out of the
 * static graph so the headless Pylon node stays bundle-able (#5037), while the
 * interactive Probe CLI still gets full TUI rendering when a human runs it.
 */
export async function loadProbeOpentuiRenderer(): Promise<ProbeOpentuiRendererModule> {
  const core = await importOpentuiCore();
  const {
    createCliRenderer,
    MarkdownRenderable,
    CodeRenderable,
    LineNumberRenderable,
    ScrollBoxRenderable,
    TextRenderable,
    BoxRenderable,
    SyntaxStyle,
    parseColor,
  } = core;

  function createDefaultSyntaxStyle(): SyntaxStyle {
    return SyntaxStyle.fromStyles({
      keyword: { fg: parseColor("#FF7B72"), bold: true },
      string: { fg: parseColor("#A5D6FF") },
      comment: { fg: parseColor("#8B949E"), italic: true },
      number: { fg: parseColor("#79C0FF") },
      function: { fg: parseColor("#D2A8FF") },
      type: { fg: parseColor("#FFA657") },
      operator: { fg: parseColor("#FF7B72") },
      variable: { fg: parseColor("#E6EDF3") },
      property: { fg: parseColor("#79C0FF") },
      bracket: { fg: parseColor("#F0F6FC") },
      delimiter: { fg: parseColor("#C9D1D9") },
      "markup.heading": { fg: parseColor("#00D7FF"), bold: true },
      "markup.bold": { fg: parseColor("#F0F6FC"), bold: true },
      "markup.italic": { fg: parseColor("#F0F6FC"), italic: true },
      "markup.list": { fg: parseColor("#FF7B72") },
      "markup.quote": { fg: parseColor("#8B949E"), italic: true },
      "markup.raw": { fg: parseColor("#A5D6FF"), bg: parseColor("#161B22") },
      "markup.link": { fg: parseColor("#58A6FF"), underline: true },
      "markup.link.url": { fg: parseColor("#58A6FF"), underline: true },
      conceal: { fg: parseColor("#6E7681") },
      default: { fg: parseColor("#E6EDF3") },
    });
  }

  function createProbeRenderer(): Promise<CliRenderer> {
    return createCliRenderer({
      exitOnCtrlC: true,
      targetFps: 30,
      screenMode: "main-screen",
    });
  }

  function createAssistantText(renderer: CliRenderer): MarkdownRenderable {
    return new MarkdownRenderable(renderer, {
      content: "",
      syntaxStyle: createDefaultSyntaxStyle(),
      conceal: true,
      internalBlockMode: "top-level",
      streaming: true,
      width: "100%",
    });
  }

  function createCodeBlock(
    renderer: CliRenderer,
    content: string,
    filetype?: string,
  ): CodeRenderable {
    return new CodeRenderable(renderer, {
      content,
      filetype: filetype ?? "plaintext",
      syntaxStyle: createDefaultSyntaxStyle(),
      width: "100%",
    });
  }

  function createCodeWithLineNumbers(
    renderer: CliRenderer,
    content: string,
    filetype?: string,
  ): LineNumberRenderable {
    const code = createCodeBlock(renderer, content, filetype);
    return new LineNumberRenderable(renderer, {
      target: code,
      minWidth: 3,
      paddingRight: 1,
      width: "100%",
    });
  }

  return {
    createProbeRenderer,
    createAssistantText,
    createCodeWithLineNumbers,
    createDefaultSyntaxStyle,
    parseColor,
    TextRenderable,
    BoxRenderable,
    ScrollBoxRenderable,
  };
}
