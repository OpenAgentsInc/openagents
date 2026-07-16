import type { ContextMeterProps } from "./context-meter.tsx";

/**
 * Fixture set for `ContextMeter` (issue 8870, epic 8857 T13 gallery lane).
 * Every value here is a plausible EXACT wire number — never a rounded or
 * guessed figure — matching the component's own "no fabricated fill" rule.
 */
export type DesktopContextMeterFixture = Readonly<{
  name: string;
  props: ContextMeterProps;
}>;

export const desktopContextMeterFixtures: ReadonlyArray<DesktopContextMeterFixture> = [
  {
    name: "no data (nothing reported yet)",
    props: { itemKey: "fixture-meter-empty" },
  },
  {
    name: "low fill, no known ceiling",
    props: {
      itemKey: "fixture-meter-low",
      usage: {
        inputTokens: 1_200,
        cachedInputTokens: 300,
        outputTokens: 140,
        reasoningTokens: 90,
        totalTokens: 1_730,
      },
    },
  },
  {
    name: "mid fill, known context-window ceiling",
    props: {
      itemKey: "fixture-meter-mid",
      usage: {
        inputTokens: 42_000,
        cachedInputTokens: 18_000,
        outputTokens: 3_200,
        reasoningTokens: 2_400,
        totalTokens: 65_600,
        contextWindowTokens: 200_000,
      },
    },
  },
  {
    name: "near limit (>=85% of ceiling)",
    props: {
      itemKey: "fixture-meter-near-limit",
      usage: {
        inputTokens: 180_000,
        outputTokens: 10_000,
        totalTokens: 190_000,
        contextWindowTokens: 200_000,
      },
    },
  },
  {
    name: "rate-limited window (100% used)",
    props: {
      itemKey: "fixture-meter-rate-limited",
      rateLimits: [{ label: "primary", usedPercent: 100 }],
    },
  },
  {
    name: "both usage and multiple rate-limit windows",
    props: {
      itemKey: "fixture-meter-combined",
      usage: { totalTokens: 8_400, contextWindowTokens: 200_000 },
      rateLimits: [
        { label: "primary", usedPercent: 62, windowDurationMins: 5 * 60 },
        { label: "secondary", usedPercent: 18, windowDurationMins: 7 * 24 * 60 },
      ],
    },
  },
  {
    name: "historical inspector snapshot",
    props: {
      itemKey: "fixture-meter-historical",
      historical: true,
      usage: { totalTokens: 4_096 },
    },
  },
];
