import {
  Badge,
  Button,
  Card,
  Image,
  IntentRef,
  Modal,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core"
import type { ChatMessageImageAttachment } from "@openagentsinc/khala-sync"

import type { MobileAccessibilityProfile } from "./khala-core"

export type MobileAttachmentPreviewState = "loading" | "ready" | "failed"

export const mobileAttachmentRef = (entryKey: string, index: number): string =>
  `${entryKey}:attachment:${index}`

const sizeLabel = (sizeBytes: number): string =>
  `${Math.max(1, Math.ceil(sizeBytes / 1024))} KB`

export const renderMobileTranscriptAttachments = (
  entryKey: string,
  attachments: ReadonlyArray<ChatMessageImageAttachment>,
  previewStates: Readonly<Record<string, MobileAttachmentPreviewState>>,
  retryEpochs: Readonly<Record<string, number>>,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => attachments.map((attachment, index) => {
  const attachmentRef = mobileAttachmentRef(entryKey, index)
  const state = previewStates[attachmentRef] ?? "loading"
  const epoch = retryEpochs[attachmentRef] ?? 0
  const source = `data:${attachment.mediaType};base64,${attachment.dataBase64}`
  return Card({
    key: `${attachmentRef}-card`,
    padding: "2",
    radius: "lg",
    style: { width: "full", borderColor: "border", borderWidth: 1, backgroundColor: "surfaceRaised" },
    a11y: { role: "region", label: `Image attachment, ${attachment.name}, ${sizeLabel(attachment.sizeBytes)}` },
  }, [
    ...(state === "failed"
      ? [Stack({ key: `${attachmentRef}-failed`, direction: "column", gap: "2", style: { width: "full" } }, [
          Text({
            key: `${attachmentRef}-failed-copy`,
            content: "Preview unavailable. The confirmed attachment is still part of this message.",
            variant: "caption",
            color: "danger",
          }),
          Button({
            key: `${attachmentRef}-retry`,
            label: "Retry preview",
            variant: "secondary",
            onPress: IntentRef("TranscriptAttachmentRetryRequested", StaticPayload({ attachmentRef })),
            style: { width: "full", minHeight: accessibility.minTouchTarget },
          }),
        ])]
      : [Image({
          key: `${attachmentRef}-image-${epoch}`,
          source,
          alt: attachment.name,
          width: "full",
          height: 220,
          fit: "cover",
          onPress: IntentRef("TranscriptAttachmentOpened", StaticPayload({ attachmentRef })),
          onLoad: IntentRef("TranscriptAttachmentLoadSettled", StaticPayload({ attachmentRef, outcome: "ready" })),
          onError: IntentRef("TranscriptAttachmentLoadSettled", StaticPayload({ attachmentRef, outcome: "failed" })),
          style: { borderRadius: "md" },
        })]),
    Stack({ key: `${attachmentRef}-meta`, direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
      Badge({
        key: `${attachmentRef}-kind`,
        label: state === "loading" ? "Loading image" : state === "failed" ? "Image unavailable" : "Image",
        tone: state === "failed" ? "danger" : state === "loading" ? "info" : "neutral",
      }),
      Text({
        key: `${attachmentRef}-caption`,
        content: `${attachment.name} · ${sizeLabel(attachment.sizeBytes)}`,
        variant: "caption",
        color: "textMuted",
        style: { flex: 1 },
      }),
    ]),
  ])
})

export const renderMobileAttachmentViewer = (
  attachmentRef: string,
  attachment: ChatMessageImageAttachment,
): View => Modal({
  key: "khala-attachment-viewer",
  title: attachment.name,
  open: true,
  dismissable: true,
  size: "full",
  onDismiss: IntentRef("TranscriptAttachmentViewerDismissed", StaticPayload({ attachmentRef })),
  a11y: { role: "dialog", label: `Image viewer, ${attachment.name}` },
}, [
  Image({
    key: `${attachmentRef}-viewer-image`,
    source: `data:${attachment.mediaType};base64,${attachment.dataBase64}`,
    alt: attachment.name,
    width: "full",
    height: "full",
    fit: "contain",
  }),
  Text({
    key: `${attachmentRef}-viewer-meta`,
    content: `${attachment.mediaType} · ${sizeLabel(attachment.sizeBytes)}`,
    variant: "caption",
    color: "textMuted",
  }),
  Button({
    key: `${attachmentRef}-viewer-close`,
    label: "Close viewer",
    variant: "secondary",
    onPress: IntentRef("TranscriptAttachmentViewerDismissed", StaticPayload({ attachmentRef })),
    style: { width: "full", minHeight: 44 },
  }),
])
