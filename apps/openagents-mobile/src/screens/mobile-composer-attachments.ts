import {
  Badge,
  Button,
  Card,
  Image,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core"
import type { CodingComposerDraftSnapshot } from "@openagentsinc/khala-sync-client"

import type { MobileAccessibilityProfile } from "./khala-core"

type DraftAttachment = CodingComposerDraftSnapshot["doc"]["attachments"][number]

const sizeLabel = (bytes: number): string => bytes < 1024
  ? `${bytes} B`
  : bytes < 1024 * 1024
    ? `${Math.max(1, Math.ceil(bytes / 1024))} KB`
    : `${(bytes / (1024 * 1024)).toFixed(1)} MB`

const stateLabel = (attachment: DraftAttachment): string => {
  switch (attachment.status) {
    case "staged": return "Preparing"
    case "uploading": return "Verifying"
    case "ready": return "Ready"
    case "error": return "Needs attention"
  }
}

export const renderMobileComposerAttachments = (
  attachments: ReadonlyArray<DraftAttachment>,
  mutatingAttachmentId: string | null,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => attachments.length === 0
  ? []
  : [Stack({
      key: "khala-coding-composer-attachments",
      direction: "column",
      gap: "1",
      style: { width: "full" },
      a11y: { role: "list", label: `${attachments.length} draft attachments` },
    }, attachments.map(attachment => {
      const mutating = attachment.id === mutatingAttachmentId
      const label = stateLabel(attachment)
      const imagePreview = attachment.kind === "image" &&
        attachment.previewUrl?.startsWith("file:") === true
      return Card({
        key: `khala-coding-composer-attachment-${attachment.id}`,
        padding: "1.5",
        radius: "md",
        style: { width: "full", borderColor: "border", borderWidth: 1, backgroundColor: "surfaceRaised" },
        a11y: {
          role: "listitem",
          label: `${attachment.kind === "image" ? "Image" : "File"} attachment, ${attachment.name}, ${sizeLabel(attachment.sizeBytes)}, ${label}`,
        },
      }, [
        Stack({
          key: `khala-coding-composer-attachment-${attachment.id}-row`,
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        }, [
          ...(imagePreview
            ? [Image({
                key: `khala-coding-composer-attachment-${attachment.id}-preview`,
                source: attachment.previewUrl!,
                alt: attachment.name,
                width: 64,
                height: 64,
                fit: "cover",
                style: { borderRadius: "sm" },
              })]
            : [Badge({
                key: `khala-coding-composer-attachment-${attachment.id}-kind`,
                label: attachment.kind === "image" ? "Image" : "File",
                tone: "neutral",
              })]),
          Stack({
            key: `khala-coding-composer-attachment-${attachment.id}-meta`,
            direction: "column",
            gap: "0.5",
            style: { flex: 1, minWidth: 0 },
          }, [
            Text({
              key: `khala-coding-composer-attachment-${attachment.id}-name`,
              content: attachment.name,
              variant: "label",
              color: "textPrimary",
            }),
            Text({
              key: `khala-coding-composer-attachment-${attachment.id}-detail`,
              content: `${sizeLabel(attachment.sizeBytes)} · ${attachment.mime} · ${mutating ? "Updating…" : label}`,
              variant: "caption",
              color: attachment.status === "error" ? "danger" : "textMuted",
            }),
            ...(attachment.status === "error" && attachment.errorText !== undefined
              ? [Text({
                  key: `khala-coding-composer-attachment-${attachment.id}-error`,
                  content: attachment.errorText,
                  variant: "caption",
                  color: "danger",
                })]
              : []),
          ]),
          ...(attachment.status === "error"
            ? [Button({
                key: `khala-coding-composer-attachment-${attachment.id}-retry`,
                label: "Retry",
                variant: "secondary",
                disabled: mutating,
                onPress: IntentRef("CodingComposerAttachmentRetryRequested", StaticPayload({ attachmentId: attachment.id })),
                style: { minHeight: accessibility.minTouchTarget },
              })]
            : []),
          Button({
            key: `khala-coding-composer-attachment-${attachment.id}-remove`,
            label: "Remove",
            variant: "ghost",
            disabled: mutating,
            onPress: IntentRef("CodingComposerAttachmentRemoved", StaticPayload({ attachmentId: attachment.id })),
            a11y: { label: `Remove ${attachment.name}` },
            style: { minHeight: accessibility.minTouchTarget },
          }),
        ]),
      ])
    }))]
