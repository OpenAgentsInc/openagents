import { Fragment, type ReactNode } from "react";
import { View, type TextStyle, type ViewStyle } from "react-native";

import { Text } from "./text";
import { colors, radius, spacing, typography } from "./theme";

/**
 * Enough Markdown for an agent transcript, rendered rather than printed.
 *
 * An agent writes headings, fenced code, lists, and inline code constantly. A
 * transcript that shows `## User` as literal characters is showing its own
 * plumbing. This covers the blocks that actually appear and leaves the rest as
 * plain text, which is the honest failure: unsupported syntax reads as the
 * words the author typed, never as a broken control.
 */

type Block =
  | Readonly<{ kind: "heading"; level: number; text: string }>
  | Readonly<{ kind: "paragraph"; text: string }>
  | Readonly<{ kind: "code"; language: string | null; text: string }>
  | Readonly<{ kind: "bullet"; text: string }>
  | Readonly<{ kind: "ordered"; marker: string; text: string }>
  | Readonly<{ kind: "quote"; text: string }>
  | Readonly<{ kind: "rule" }>;

const HEADING = /^(#{1,6})\s+(.*)$/;
const BULLET = /^\s*[-*+]\s+(.*)$/;
const ORDERED = /^\s*(\d{1,3})[.)]\s+(.*)$/;
const QUOTE = /^\s*>\s?(.*)$/;
const RULE = /^\s*(-{3,}|\*{3,}|_{3,})\s*$/;
const FENCE = /^\s*```\s*([A-Za-z0-9+#._-]*)\s*$/;

export const parseMarkdown = (source: string): ReadonlyArray<Block> => {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let paragraph: string[] = [];

  const flush = (): void => {
    if (paragraph.length === 0) return;
    blocks.push({ kind: "paragraph", text: paragraph.join("\n").trim() });
    paragraph = [];
  };

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    const fence = FENCE.exec(line);
    if (fence !== null) {
      flush();
      const language = fence[1] === undefined || fence[1] === "" ? null : fence[1];
      const body: string[] = [];
      index += 1;
      while (index < lines.length && FENCE.exec(lines[index] ?? "") === null) {
        body.push(lines[index] ?? "");
        index += 1;
      }
      blocks.push({ kind: "code", language, text: body.join("\n") });
      continue;
    }
    if (line.trim() === "") {
      flush();
      continue;
    }
    if (RULE.test(line)) {
      flush();
      blocks.push({ kind: "rule" });
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading !== null) {
      flush();
      blocks.push({
        kind: "heading",
        level: (heading[1] ?? "#").length,
        text: heading[2] ?? "",
      });
      continue;
    }
    const bullet = BULLET.exec(line);
    if (bullet !== null) {
      flush();
      blocks.push({ kind: "bullet", text: bullet[1] ?? "" });
      continue;
    }
    const ordered = ORDERED.exec(line);
    if (ordered !== null) {
      flush();
      blocks.push({ kind: "ordered", marker: ordered[1] ?? "1", text: ordered[2] ?? "" });
      continue;
    }
    const quote = QUOTE.exec(line);
    if (quote !== null) {
      flush();
      blocks.push({ kind: "quote", text: quote[1] ?? "" });
      continue;
    }
    paragraph.push(line);
  }
  flush();
  return blocks;
};

type Span = Readonly<{ text: string; code?: boolean; bold?: boolean; italic?: boolean; link?: boolean }>;

const INLINE = /(`[^`]+`)|(\*\*[^*]+\*\*)|(__[^_]+__)|(\*[^*\n]+\*)|(\[[^\]]+\]\([^)]+\))/;

export const parseInline = (source: string): ReadonlyArray<Span> => {
  const spans: Span[] = [];
  let rest = source;
  for (let guard = 0; guard < 400 && rest !== ""; guard += 1) {
    const match = INLINE.exec(rest);
    if (match === null || match.index === undefined) break;
    if (match.index > 0) spans.push({ text: rest.slice(0, match.index) });
    const token = match[0];
    if (token.startsWith("`")) spans.push({ text: token.slice(1, -1), code: true });
    else if (token.startsWith("**") || token.startsWith("__"))
      spans.push({ text: token.slice(2, -2), bold: true });
    else if (token.startsWith("*")) spans.push({ text: token.slice(1, -1), italic: true });
    else {
      const label = /\[([^\]]+)\]/.exec(token);
      spans.push({ text: label?.[1] ?? token, link: true });
    }
    rest = rest.slice(match.index + token.length);
  }
  if (rest !== "") spans.push({ text: rest });
  return spans.length === 0 ? [{ text: source }] : spans;
};

const Inline = ({ source, style }: { readonly source: string; readonly style?: TextStyle }) => (
  <Text preset="body" style={style}>
    {parseInline(source).map((span, index) => (
      <Text
        key={`${index}-${span.text.slice(0, 8)}`}
        preset={span.code === true ? "mono" : "body"}
        color={
          span.link === true
            ? colors.accentInk
            : span.code === true
              ? colors.accentHot
              : undefined
        }
        style={[
          span.bold === true ? $bold : null,
          span.italic === true ? $italic : null,
          span.code === true ? $inlineCode : null,
        ]}
      >
        {span.text}
      </Text>
    ))}
  </Text>
);

export const Markdown = ({ source }: { readonly source: string }) => {
  const blocks = parseMarkdown(source);
  return (
    <View style={$stack}>
      {blocks.map((block, index) => (
        <Fragment key={index}>{renderBlock(block)}</Fragment>
      ))}
    </View>
  );
};

const renderBlock = (block: Block): ReactNode => {
  switch (block.kind) {
    case "heading":
      return (
        <Text preset={block.level <= 2 ? "subheading" : "bodyStrong"} style={$heading}>
          {block.text}
        </Text>
      );
    case "code":
      return (
        <View style={$code}>
          {block.language === null ? null : (
            <Text preset="caption" color={colors.textFaint} style={$codeLanguage}>
              {block.language}
            </Text>
          )}
          <Text preset="mono" color={colors.textBody}>
            {block.text}
          </Text>
        </View>
      );
    case "bullet":
      return (
        <View style={$listRow}>
          <Text preset="body" color={colors.accent} style={$listMarker}>
            ·
          </Text>
          <Inline source={block.text} style={$listBody} />
        </View>
      );
    case "ordered":
      return (
        <View style={$listRow}>
          <Text preset="mono" color={colors.accent} style={$listMarker}>
            {block.marker}
          </Text>
          <Inline source={block.text} style={$listBody} />
        </View>
      );
    case "quote":
      return (
        <View style={$quote}>
          <Inline source={block.text} style={$quoteBody} />
        </View>
      );
    case "rule":
      return <View style={$rule} />;
    case "paragraph":
      return <Inline source={block.text} />;
  }
};

const $stack: ViewStyle = { gap: spacing.extraSmall };
// A bundled family carries its own weight and slant files, so naming the face
// is what actually changes the glyphs. `fontWeight` alone would leave the
// regular file in place and emphasis would disappear.
const $bold: TextStyle = { fontFamily: typography.sansSemiBold, color: colors.text };
const $italic: TextStyle = { fontFamily: "IBMPlexSans-Italic" };
const $inlineCode: TextStyle = { fontFamily: typography.mono };
const $heading: TextStyle = { color: colors.text, marginTop: spacing.micro };
const $code: ViewStyle = {
  backgroundColor: colors.surfaceSunken,
  borderRadius: radius.small,
  borderWidth: 1,
  borderColor: colors.border,
  padding: spacing.small,
  gap: spacing.micro,
};
const $codeLanguage: TextStyle = { textTransform: "lowercase" };
const $listRow: ViewStyle = { flexDirection: "row", gap: spacing.extraSmall };
const $listMarker: TextStyle = { minWidth: 14, textAlign: "right" };
const $listBody: TextStyle = { flex: 1 };
const $quote: ViewStyle = {
  paddingLeft: spacing.small,
  borderLeftWidth: 1,
  borderLeftColor: colors.borderEnergized,
};
const $quoteBody: TextStyle = { color: colors.textDim };
const $rule: ViewStyle = { height: 1, backgroundColor: colors.border, marginVertical: spacing.tiny };

/**
 * A one-line plain rendering, for a preview that has no room to lay out
 * blocks. Strips the syntax rather than printing it, so a list preview reads
 * as words instead of as `##` and backticks.
 */
export const markdownToPlainText = (source: string): string =>
  source
    .replace(/```[\s\S]*?```/g, " code ")
    .replace(/^\s{0,3}#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*\d{1,3}[.)]\s+/gm, "")
    .replace(/^\s*>\s?/gm, "")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/__([^_]+)__/g, "$1")
    .replace(/\*([^*\n]+)\*/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
