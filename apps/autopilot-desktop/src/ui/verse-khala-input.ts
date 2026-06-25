// In-world Khala textbox (EPIC #6017 — talk to Khala from a Verse textbox).
//
// A Foldkit HUD surface anchored OVER the rendered 3D Verse scene: a one-line
// input bar + a live response bubble + a receipt line. The user types a prompt,
// hits Enter (or Send), and Khala answers — tokens stream into the bubble live,
// and the LOCAL crackling-arc effect fires from the Khala nexus to the avatar the
// moment a real receipt lands (driven in view.ts via withVerseKhalaEffectLayer).
//
// This is a pure projection of the model's `verseKhala*` fields into Html; the
// streaming, token resolution, and receipt all live in the Bun host + reducer.
// Honest states: 402 add-credit / no-token / errors arrive as the response text +
// an error-toned status (the Bun host never fabricates an answer). No receipt ⇒
// no effect, and the receipt line says "no receipt — unverified".

import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import { Option } from "effect"

import type { Message } from "./message.js"
import {
  ChangedVerseKhalaInput,
  SubmittedVerseKhala,
} from "./message.js"
import type { Model } from "./model.js"
import { modelVerseKhalaReceipt } from "./model.js"
import { summarizeKhalaReceipt } from "../shared/khala-cockpit.js"

const h = html<Message>()
const cls = (value: string): Attribute<Message> => h.Class(value)

const shortRef = (value: string): string =>
  value.length <= 40 ? value : `${value.slice(0, 18)}…${value.slice(-12)}`

const issuerLabel = (value: Model["verseKhalaIssuerPath"]): string =>
  value === "pylon_mcp_local"
    ? "local Pylon MCP"
    : value === "remote_mcp"
      ? "remote MCP"
      : "gateway"

const verseKhalaDurableHandleLine = (model: Model): Html => {
  const hasHandle =
    model.verseKhalaDurableRequestId !== null ||
    model.verseKhalaDurableStreamUrl !== null ||
    model.verseKhalaAssignmentRef !== null
  const issuerPath = model.verseKhalaIssuerPath
  if (!hasHandle && (issuerPath === null || issuerPath === "legacy_gateway")) {
    return h.empty
  }
  const pieces = [
    issuerLabel(issuerPath),
    model.verseKhalaDurableRequestId === null
      ? null
      : `resume ${shortRef(model.verseKhalaDurableRequestId)}`,
    model.verseKhalaAssignmentRef === null
      ? null
      : `assignment ${shortRef(model.verseKhalaAssignmentRef)}`,
  ].filter((piece): piece is string => piece !== null)
  return h.p(
    [
      cls("verse-khala-durable"),
      h.DataAttribute("verse-khala-durable", issuerPath ?? "legacy_gateway"),
      ...(model.verseKhalaDurableRequestId === null
        ? []
        : [
            h.DataAttribute(
              "verse-khala-durable-request-id",
              model.verseKhalaDurableRequestId,
            ),
          ]),
      ...(model.verseKhalaDurableStreamUrl === null
        ? []
        : [
            h.DataAttribute(
              "verse-khala-durable-stream-url",
              model.verseKhalaDurableStreamUrl,
            ),
          ]),
      ...(model.verseKhalaAssignmentRef === null
        ? []
        : [
            h.DataAttribute(
              "verse-khala-assignment-ref",
              model.verseKhalaAssignmentRef,
            ),
          ]),
    ],
    [pieces.join(" — ")],
  )
}

// The receipt line: a short, public-safe one-liner. With a real receipt it shows
// the served model / lane / verification / live; with none, "no receipt".
const verseKhalaReceiptLine = (model: Model): Html => {
  const receipt = modelVerseKhalaReceipt(model)
  if (receipt === null && !model.verseKhalaInFlight && model.verseKhalaResponse === "") {
    return h.empty
  }
  const text = summarizeKhalaReceipt(receipt)
  const tone = receipt !== null && receipt.receipt !== null ? "live" : "none"
  return h.p(
    [
      cls(`verse-khala-receipt verse-khala-receipt-${tone}`),
      h.DataAttribute("verse-khala-receipt", tone),
      ...(receipt?.receipt === undefined || receipt?.receipt === null
        ? []
        : [h.DataAttribute("verse-khala-receipt-ref", receipt.receipt)]),
    ],
    [text],
  )
}

// The live response bubble: appends token deltas as they stream in. Hidden until
// there is something to show (a turn in flight, or a landed response).
const verseKhalaResponseBubble = (model: Model): Html => {
  const visible =
    model.verseKhalaInFlight || model.verseKhalaResponse.trim().length > 0
  if (!visible) return h.empty
  const body =
    model.verseKhalaResponse.trim().length > 0
      ? model.verseKhalaResponse
      : "…"
  return h.div(
    [
      cls("verse-khala-bubble"),
      h.DataAttribute(
        "verse-khala-bubble",
        model.verseKhalaInFlight ? "streaming" : "settled",
      ),
    ],
    [
      h.span([cls("verse-khala-bubble-speaker mono")], ["Khala"]),
      h.p([cls("verse-khala-bubble-body")], [body]),
    ],
  )
}

// The one-line input bar + Send button. Enter submits (Shift+Enter inserts a
// newline only in a textarea; this is a single-line input, so Enter always
// submits). The input stays enabled while a turn is in flight (the reducer
// no-ops a concurrent submit) so focus is never dropped mid-stream.
const verseKhalaInputBar = (model: Model): Html =>
  h.div(
    [cls("verse-khala-bar"), h.DataAttribute("verse-khala-bar", "ready")],
    [
      h.input([
        cls("verse-khala-input"),
        h.Type("text"),
        h.Placeholder("Talk to Khala…"),
        h.Value(model.verseKhalaInput),
        h.AriaLabel("Talk to Khala"),
        h.OnInput((value: string) => ChangedVerseKhalaInput({ value })),
        h.OnKeyDownPreventDefault((key, mods) =>
          key === "Enter" && !mods.shiftKey
            ? Option.some(SubmittedVerseKhala())
            : Option.none(),
        ),
      ]),
      h.button(
        [
          cls("verse-khala-send"),
          h.Type("button"),
          h.Disabled(model.verseKhalaInFlight),
          h.OnClick(SubmittedVerseKhala()),
        ],
        // "Ask" (not "Send") — the legacy chat-composer "Send" chrome is
        // intentionally absent from the Verse first paint (verse-launch-checklist).
        [model.verseKhalaInFlight ? "Asking…" : "Ask"],
      ),
    ],
  )

// The status line (info/error), including the honest 402 add-credit message that
// the Bun host returns as the response text + an error tone here.
const verseKhalaStatusLine = (model: Model): Html => {
  if (model.verseKhalaStatus.tone === "idle") return h.empty
  return h.p(
    [
      cls(`verse-khala-status verse-khala-status-${model.verseKhalaStatus.tone}`),
      h.DataAttribute("verse-khala-status", model.verseKhalaStatus.tone),
    ],
    [model.verseKhalaStatus.text],
  )
}

// The whole in-world Khala overlay panel, anchored over the Verse scene.
export const verseKhalaInputOverlay = (model: Model): Html =>
  h.div(
    [
      cls("verse-khala-overlay"),
      h.DataAttribute(
        "verse-khala-overlay",
        model.verseKhalaInFlight ? "in-flight" : "idle",
      ),
    ],
    [
      verseKhalaResponseBubble(model),
      verseKhalaDurableHandleLine(model),
      verseKhalaReceiptLine(model),
      verseKhalaStatusLine(model),
      verseKhalaInputBar(model),
    ],
  )
