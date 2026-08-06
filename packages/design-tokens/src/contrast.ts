import { toneStateTokens, toneTokens, toneVariantTokens } from "./index.ts";
import type { Theme } from "./index.ts";

export const WCAG_AA_NORMAL_TEXT_RATIO = 4.5;

export interface ContrastViolation {
  readonly foregroundRole: string;
  readonly backgroundRole: string;
  readonly foreground: string;
  readonly background: string;
  readonly ratio: number;
  readonly minimum: number;
}

const channel = (value: number): number => {
  const normalized = value / 255;
  return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
};

const opaqueRgb = (color: string): readonly [number, number, number] => {
  const value = color.slice(1);
  if (value.length === 3) {
    const component = (index: number) => Number.parseInt(value[index]!.repeat(2), 16);
    return [component(0), component(1), component(2)];
  }
  if (value.length !== 6) {
    throw new Error(`Contrast checks require opaque hex colors, received ${color}`);
  }
  return [
    Number.parseInt(value.slice(0, 2), 16),
    Number.parseInt(value.slice(2, 4), 16),
    Number.parseInt(value.slice(4, 6), 16),
  ];
};

export const relativeLuminance = (color: string): number => {
  const [red, green, blue] = opaqueRgb(color);
  return 0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue);
};

export const contrastRatio = (foreground: string, background: string): number => {
  const light = Math.max(relativeLuminance(foreground), relativeLuminance(background));
  const dark = Math.min(relativeLuminance(foreground), relativeLuminance(background));
  return (light + 0.05) / (dark + 0.05);
};

const compositeOver = (color: string, background: string): string => {
  if (color.length !== 9) return color;
  const alpha = Number.parseInt(color.slice(7, 9), 16) / 255;
  const source = opaqueRgb(color.slice(0, 7));
  const base = opaqueRgb(background);
  const mix = (sourceValue: number, baseValue: number) =>
    Math.round(sourceValue * alpha + baseValue * (1 - alpha));
  const mixed = [
    mix(source[0], base[0]),
    mix(source[1], base[1]),
    mix(source[2], base[2]),
  ];
  return `#${mixed.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
};

const surfaceRoles = ["background", "surface", "surfaceRaised", "surfaceOverlay"] as const;
const textRoles = ["textPrimary", "textBody", "textMuted", "textFaint"] as const;
const attentionRoles = [
  "attentionApproval",
  "attentionInput",
  "attentionWorking",
  "attentionFailed",
  "attentionDone",
] as const;
const codeTextRoles = [
  "diffAdd",
  "diffRemove",
  "syntaxKeyword",
  "syntaxString",
  "syntaxComment",
  "syntaxFunction",
  "syntaxNumber",
  "syntaxOperator",
] as const;

export const themeContrastViolations = (
  theme: Theme,
  minimum = WCAG_AA_NORMAL_TEXT_RATIO,
): ReadonlyArray<ContrastViolation> => {
  const requirements: Array<readonly [string, string, string, string]> = [];
  for (const foregroundRole of [...textRoles, ...attentionRoles]) {
    for (const backgroundRole of surfaceRoles) {
      requirements.push([
        foregroundRole,
        backgroundRole,
        theme.color[foregroundRole],
        theme.color[backgroundRole],
      ]);
    }
  }
  requirements.push(["textInverse", "accent", theme.color.textInverse, theme.color.accent]);
  for (const foregroundRole of codeTextRoles) {
    requirements.push([
      foregroundRole,
      "codeBackground",
      theme.color[foregroundRole],
      theme.color.codeBackground,
    ]);
  }
  for (const tone of toneTokens) {
    for (const variant of toneVariantTokens) {
      for (const state of toneStateTokens) {
        if (state === "disabled") continue;
        const cell = theme.colorMatrix[tone][variant][state];
        const backgrounds =
          cell.background.length === 9
            ? surfaceRoles.map(
                (surfaceRole) =>
                  [surfaceRole, compositeOver(cell.background, theme.color[surfaceRole])] as const,
              )
            : [[`${tone}.${variant}.${state}.background`, cell.background] as const];
        for (const [backgroundRole, background] of backgrounds) {
          requirements.push([
            `colorMatrix.${tone}.${variant}.${state}.text`,
            backgroundRole,
            cell.text,
            background,
          ]);
        }
      }
    }
  }

  return requirements.flatMap(([foregroundRole, backgroundRole, foreground, background]) => {
    const ratio = contrastRatio(foreground, background);
    return ratio + Number.EPSILON < minimum
      ? [
          {
            foregroundRole,
            backgroundRole,
            foreground,
            background,
            ratio,
            minimum,
          },
        ]
      : [];
  });
};

export const assertThemeContrast = (name: string, theme: Theme): void => {
  const violations = themeContrastViolations(theme);
  if (violations.length === 0) return;
  const details = violations
    .map(
      (violation) =>
        `${violation.foregroundRole} ${violation.foreground} on ${violation.backgroundRole} ${violation.background}: ${violation.ratio.toFixed(2)}:1 < ${violation.minimum}:1`,
    )
    .join("\n");
  throw new Error(`${name} violates WCAG AA:\n${details}`);
};
