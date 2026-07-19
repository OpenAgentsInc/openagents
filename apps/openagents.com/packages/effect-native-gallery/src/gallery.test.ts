import { describe, expect, test } from "vite-plus/test";
import { componentTags } from "@effect-native/core";
import {
  khalaUiCapabilityMatrix,
  khalaUiMotifIds,
  khalaUiRendererIds,
  missingKhalaUiCapabilityDispositions,
} from "./khala-ui-contract.js";
import {
  activeStory,
  allStories,
  applyStoryControlValue,
  applyStoryControlValues,
  componentPageId,
  defaultStorybook,
  galleryPageById,
  galleryPageCoverage,
  galleryPages,
  initialGalleryState,
  parseStorybook,
  parseStory,
  serializeStorybook,
  serializeStory,
  storiesForComponent,
  storyById,
  storyCoverage,
} from "./index.js";

describe("serialization round-trips", () => {
  test("parseStory(serializeStory(s)) deep-equals the original story", () => {
    const original = storyById("stack-column");
    expect(original).toBeDefined();
    const roundTripped = parseStory(serializeStory(original!));
    expect(roundTripped).toEqual(original);
  });

  test("serializeStory produces indented JSON that parses to the same id and component", () => {
    const original = storyById("text-body")!;
    const json = serializeStory(original);
    // Two-space indentation from JSON.stringify(..., null, 2).
    expect(json).toContain('\n  "id": "text-body"');
    const parsed = JSON.parse(json) as { id: string; component: string };
    expect(parsed.id).toBe("text-body");
    expect(parsed.component).toBe("Text");
  });

  test("parseStory rejects malformed input (missing required fields)", () => {
    expect(() => parseStory('{"bad":true}')).toThrow();
  });

  test("parseStory rejects a story with an unknown component tag", () => {
    const original = storyById("stack-column")!;
    const mutated = { ...JSON.parse(serializeStory(original)), component: "NotAComponent" };
    expect(() => parseStory(JSON.stringify(mutated))).toThrow();
  });

  test("parseStorybook(serializeStorybook(defaultStorybook)) deep-equals the default storybook", () => {
    const roundTripped = parseStorybook(serializeStorybook(defaultStorybook));
    expect(roundTripped).toEqual(defaultStorybook);
  });

  test("parseStorybook rejects malformed input", () => {
    expect(() => parseStorybook("not json at all")).toThrow();
    expect(() => parseStorybook('{"version":"wrong"}')).toThrow();
  });
});

describe("story control application", () => {
  test("applyStoryControlValue rewrites both the control value and the view path", () => {
    const original = storyById("stack-column")!;
    expect((original.view as { direction: string }).direction).toBe("column");

    const applied = applyStoryControlValue(original, "stack-direction", "row");
    expect((applied.view as { direction: string }).direction).toBe("row");
    expect(applied.controls.find((c) => c.id === "stack-direction")!.value).toBe("row");

    // Original story is not mutated.
    expect((original.view as { direction: string }).direction).toBe("column");
    expect(original.controls.find((c) => c.id === "stack-direction")!.value).toBe("column");
  });

  test("applyStoryControlValue with an unknown control id returns the same story reference", () => {
    const original = storyById("stack-column")!;
    const applied = applyStoryControlValue(original, "does-not-exist", "row");
    expect(applied).toBe(original);
  });

  test("applyStoryControlValues applies every entry left-to-right", () => {
    const original = storyById("stack-column")!;
    const applied = applyStoryControlValues(original, {
      "stack-direction": "row",
      "stack-gap": "4",
    });
    expect((applied.view as { direction: string; gap: string }).direction).toBe("row");
    expect((applied.view as { direction: string; gap: string }).gap).toBe("4");
    expect(applied.controls.find((c) => c.id === "stack-direction")!.value).toBe("row");
    expect(applied.controls.find((c) => c.id === "stack-gap")!.value).toBe("4");
  });
});

describe("story and page queries over the built-in storybook", () => {
  test("allStories returns exactly 108 stories across 79 covered components", () => {
    expect(allStories().length).toBe(108);
    const coverage = storyCoverage();
    expect(coverage.covered.length).toBe(componentTags.length);
    expect(coverage.covered.length).toBe(79);
    expect(coverage.missing).toEqual([]);
  });

  test("storiesForComponent('Stack') returns the two known Stack stories in order", () => {
    const stories = storiesForComponent("Stack");
    expect(stories.map((s) => s.id)).toEqual(["stack-column", "stack-responsive"]);
  });

  test("storiesForComponent returns an empty array for a component with no stories query miss", () => {
    // Every catalog tag is covered, so exercise the empty branch through a made-up tag.
    expect(storiesForComponent("NotAComponent" as never)).toEqual([]);
  });

  test("storyById resolves a known id and returns undefined for an unknown id", () => {
    const found = storyById("stack-column");
    expect(found?.id).toBe("stack-column");
    expect(found?.component).toBe("Stack");
    expect(storyById("no-such-story")).toBeUndefined();
  });

  test("galleryPages has 6 foundation pages plus one component page per tag", () => {
    expect(galleryPages.length).toBe(6 + componentTags.length);
    expect(galleryPages.length).toBe(85);
  });

  test("componentPageId formats the component: prefix and galleryPageById resolves both kinds", () => {
    expect(componentPageId("Stack")).toBe("component:Stack");
    expect(galleryPageById("component:Stack")?.kind).toBe("component");
    expect(galleryPageById("component:Stack")?.title).toBe("Stack");
    expect(galleryPageById("design-tokens")?.kind).toBe("foundation");
    expect(galleryPageById("no-such-page")).toBeUndefined();
  });

  test("galleryPageCoverage reports every component tag as covered", () => {
    const coverage = galleryPageCoverage();
    expect(coverage.missing).toEqual([]);
    expect(coverage.covered.length).toBe(componentTags.length);
  });
});

describe("initial gallery state and active story", () => {
  test("initialGalleryState selects the first component and story with default theme/viewport", () => {
    const state = initialGalleryState();
    expect(state.activeComponent).toBe("Stack");
    expect(state.activeStoryId).toBe("stack-column");
    expect(state.activeThemeId).toBe("default");
    expect(state.activeViewportId).toBe("desktop");
    expect(state.activePageId).toBe("");
    expect(state.controlValues).toEqual({});
    expect(state.pressedCount).toBe(0);
  });

  test("activeStory resolves the selected story and applies stored control overrides", () => {
    const state = initialGalleryState();
    expect(activeStory(state).id).toBe("stack-column");
    // Base story keeps its authored direction with no overrides.
    expect((activeStory(state).view as { direction: string }).direction).toBe("column");

    const overridden = {
      ...state,
      controlValues: { "stack-column": { "stack-direction": "row" } },
    };
    expect((activeStory(overridden).view as { direction: string }).direction).toBe("row");
  });
});

describe("khala-ui capability matrix", () => {
  test("matrix disposition invariant holds for every motif and renderer", () => {
    for (const motif of khalaUiMotifIds) {
      const row = khalaUiCapabilityMatrix[motif];
      expect(row.headless.disposition).toBe("supported");
      expect(row.dom.disposition).toBe("supported");
      expect(row["react-dom"].disposition).toBe("supported");
      expect(row.canvas.disposition).toBe("unavailable");
      // Only radial-dial degrades on react-native; every other motif is supported.
      expect(row["react-native"].disposition).toBe(
        motif === "radial-dial" ? "degraded" : "supported",
      );
    }
  });

  test("missingKhalaUiCapabilityDispositions is empty for the full motif x renderer set", () => {
    expect(
      missingKhalaUiCapabilityDispositions(
        khalaUiMotifIds,
        khalaUiRendererIds,
        khalaUiCapabilityMatrix,
      ),
    ).toEqual([]);
  });

  test("missingKhalaUiCapabilityDispositions reports only the unknown motif/renderer pairs", () => {
    expect(
      missingKhalaUiCapabilityDispositions(
        ["cut-corner-surface"],
        ["dom", "fake-renderer"],
        khalaUiCapabilityMatrix,
      ),
    ).toEqual(["cut-corner-surface:fake-renderer"]);

    expect(
      missingKhalaUiCapabilityDispositions(["ghost-motif"], ["dom"], khalaUiCapabilityMatrix),
    ).toEqual(["ghost-motif:dom"]);
  });
});
