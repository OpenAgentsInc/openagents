import {
  Button,
  Card,
  CodeBlock,
  CopyButton,
  Image,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core"

import {
  MOBILE_REPOSITORY_MAX_DEPTH,
  type MobileRepositoryBrowserState,
  type MobileRepositoryTreeEntry,
} from "../coding/mobile-repository-files"
import type { MobileAccessibilityProfile } from "./khala-core"
import { mobileRichContentViews } from "./mobile-transcript-content"

const sizeLabel = (bytes: number | null): string => bytes === null
  ? ""
  : bytes < 1_024
    ? `${bytes} B`
    : `${Math.max(1, Math.ceil(bytes / 1_024))} KB`

const sourceLines = (source: string) => source.split("\n").map(line => ({
  tokens: [{ kind: "plain" as const, text: line }],
}))

const indentation = (depth: number): "0" | "2" | "4" | "6" | "8" | "10" | "12" =>
  (["0", "2", "4", "6", "8", "10", "12"] as const)[Math.min(6, depth)] ?? "12"

const repositoryRows = (
  state: MobileRepositoryBrowserState,
  directoryRef = "",
  depth = 0,
): ReadonlyArray<View> => {
  if (depth > MOBILE_REPOSITORY_MAX_DEPTH) return []
  const page = state.pages[directoryRef]
  if (page === undefined) return []
  return page.entries.flatMap((entry: MobileRepositoryTreeEntry): ReadonlyArray<View> => {
    const expanded = entry.kind === "directory" && state.expandedRefs.includes(entry.pathRef)
    const selected = state.preview.state === "ready" && state.preview.preview.pathRef === entry.pathRef
    const row = Button({
      key: `repository-file-${entry.pathRef}`,
      label: `${entry.kind === "directory" ? expanded ? "▾" : "›" : ""} ${entry.name}${entry.kind === "file" && entry.sizeBytes !== null ? ` · ${sizeLabel(entry.sizeBytes)}` : ""}`.trim(),
      variant: selected ? "secondary" : "ghost",
      selected,
      onPress: IntentRef(
        entry.kind === "directory" ? "RepositoryDirectoryToggled" : "RepositoryFileSelected",
        StaticPayload({ pathRef: entry.pathRef, revisionRef: entry.revisionRef }),
      ),
      a11y: {
        role: "treeitem",
        label: entry.kind === "directory"
          ? `${expanded ? "Collapse" : "Expand"} folder ${entry.pathRef}`
          : `Preview file ${entry.pathRef}${entry.sizeBytes === null ? "" : `, ${sizeLabel(entry.sizeBytes)}`}`,
        ...(entry.kind === "directory" ? { expanded } : {}),
      },
      style: { width: "full", minHeight: 44, paddingLeft: indentation(depth) },
    })
    return expanded ? [row, ...repositoryRows(state, entry.pathRef, depth + 1)] : [row]
  })
}

const previewView = (
  state: MobileRepositoryBrowserState,
  accessibility: MobileAccessibilityProfile,
): View => {
  const preview = state.preview
  if (preview.state === "idle") return Text({
    key: "repository-preview-idle",
    content: "Choose a source, Markdown, or image file to preview it safely.",
    variant: "body",
    color: "textMuted",
  })
  if (preview.state === "loading") return Text({
    key: "repository-preview-loading",
    content: `Loading ${preview.pathRef}…`,
    variant: "body",
    color: "textMuted",
  })
  if (preview.state === "failed") return Stack({
    key: "repository-preview-failed",
    direction: "column",
    gap: "2",
  }, [
    Text({ key: "repository-preview-failed-title", content: preview.pathRef, variant: "heading" }),
    Text({ key: "repository-preview-failed-message", content: preview.message, variant: "body", color: "danger" }),
  ])

  const file = preview.preview
  const header = Stack({ key: "repository-preview-header", direction: "column", gap: "1" }, [
    Text({ key: "repository-preview-path", content: file.pathRef, variant: "heading" }),
    Text({
      key: "repository-preview-meta",
      content: `${file.kind === "source" ? file.language : file.kind} · ${sizeLabel(file.sizeBytes)} · ${file.revisionRef}`,
      variant: "caption",
      color: "textMuted",
    }),
    CopyButton({
      key: "repository-copy-path",
      content: file.pathRef,
      label: "Copy path",
      accessibilityLabel: `Copy path ${file.pathRef}`,
      size: "sm",
      variant: "ghost",
    }),
  ])
  const content = file.kind === "image"
    ? [Image({
        key: `repository-image-${file.sha256}`,
        source: file.contentUrl,
        alt: `Preview of ${file.pathRef}`,
        width: "full",
        height: 320,
        fit: "contain",
        style: { borderRadius: "md" },
      })]
    : file.kind === "markdown"
      ? mobileRichContentViews("repository-markdown", file.content, `Copy ${file.pathRef}`)
      : [
          CodeBlock({
            key: "repository-source-code",
            language: file.language,
            lines: sourceLines(file.content),
            showLineNumbers: true,
            style: { width: "full", borderRadius: "md", padding: "2" },
          }),
          CopyButton({
            key: "repository-copy-source",
            content: file.content,
            label: "Copy file",
            accessibilityLabel: `Copy contents of ${file.pathRef}`,
            size: "sm",
            variant: "ghost",
          }),
        ]
  return Card({
    key: "repository-preview-ready",
    padding: "3",
    radius: "lg",
    a11y: { role: "region", label: `File preview, ${file.pathRef}` },
    style: { width: "full", borderWidth: 1, borderColor: "border", backgroundColor: "surfaceRaised" },
  }, [header, ...content, Button({
    key: "repository-return-conversation-bottom",
    label: "Return to conversation",
    variant: "secondary",
    onPress: IntentRef("FilesRouteClosed", StaticPayload({})),
    style: { width: "full", minHeight: accessibility.minTouchTarget },
  })])
}

export const renderMobileFilesView = (
  state: MobileRepositoryBrowserState,
  accessibility: MobileAccessibilityProfile,
): View => Stack({
  key: "mobile-files-root",
  direction: "column",
  gap: "3",
  padding: "3",
  preserveScrollAnchor: true,
  style: { width: "full", height: "full", backgroundColor: "background" },
  a11y: { role: "region", label: "Repository files" },
}, [
  Stack({ key: "repository-files-actions", direction: "row", gap: "2", align: "center" }, [
    Button({
      key: "repository-return-conversation",
      label: "Conversation",
      variant: "ghost",
      onPress: IntentRef("FilesRouteClosed", StaticPayload({})),
      style: { minHeight: accessibility.minTouchTarget },
    }),
    Button({
      key: "repository-refresh",
      label: "Refresh files",
      variant: "ghost",
      disabled: state.state === "loading" || state.scope === null,
      onPress: IntentRef("RepositoryFilesRefreshed", StaticPayload({})),
      style: { minHeight: accessibility.minTouchTarget },
    }),
  ]),
  ...(state.scope === null ? [] : [Text({
    key: "repository-scope",
    content: `${state.scope.repositoryRef} · ${state.scope.worktreeRef}`,
    variant: "caption",
    color: "textMuted",
  })]),
  ...(state.state === "loading" ? [Text({ key: "repository-loading", content: "Loading repository files…", variant: "body", color: "textMuted" })] : []),
  ...(state.state === "unavailable" || state.state === "failed" ? [Text({
    key: "repository-unavailable",
    content: state.message ?? "Repository files are unavailable.",
    variant: "body",
    color: state.state === "failed" ? "danger" : "textMuted",
  })] : []),
  ...(state.state === "ready" && (state.pages[""]?.entries.length ?? 0) === 0
    ? [Text({ key: "repository-empty", content: "This worktree has no visible files.", variant: "body", color: "textMuted" })]
    : repositoryRows(state)),
  previewView(state, accessibility),
])
