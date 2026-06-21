// #5467 (EPIC #5461): the Autonomous loop view — a first-class, read-only
// projection of the autonomous coordinator loop (intent → plan → fanout →
// reconcile → ship). It lives in the Supervise nav group (registered via the
// nav seam in nav.ts; one PaneId + one paneView case + one NavDestination).
//
// What it shows, all from data the control protocol ALREADY exposes:
//   - a header roll-up + the existing coordinator Pause/Resume (reuses the
//     `ClickedCoordinatorToggle` message — NO new control verb);
//   - the intent queue (`intent.list`), each ask with its live status;
//   - per-intent, the five loop stages derived from the REAL status +
//     statusHistory (intent → plan → fanout → reconcile → ship);
//   - the ship-gate state stated HONESTLY (default-DENY → escalates; never an
//     implied autonomous spend);
//   - the node's active coding sessions as the fan-out work pool, each linking
//     to session-detail (`SelectedSession`).
//
// Read-only: this pane adds NO new spend or execution authority. It does not
// fabricate a per-intent → session mapping the node does not publish (the
// coordinator's intentId→sessionRefs map is not on the control API); it shows
// the loop progression honestly and links to the live sessions separately.
// Refs-only / public-safe: it renders titleRef / intentId suffixes / session
// refs, never raw command text or secrets.

import type { Html } from "foldkit/html"
import { html } from "foldkit/html"

import {
  autonomousLoopSummary,
  coordinatorToggleLabel,
  loopStageStates,
  shipGateLine,
  shipStatusLine,
} from "./helpers.js"
import { ClickedCoordinatorToggle, type Message, SelectedSession } from "./message.js"
import { type Model, modelNode } from "./model.js"
import type { IntentRow } from "../shared/rpc.js"
import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"

const h = html<Message>()
const cls = (value: string) => h.Class(value)

// Refs-only label for an ask: prefer its (already public-safe) titleRef, else
// the short intentId suffix. Never the plaintext body.
const intentLabel = (intent: IntentRow): string => {
  const title = intent.title.trim()
  if (title !== "") return title
  return `ask ${intent.intentId.slice(-8)}`
}

const ACTIVE_SESSION_STATES = new Set(["queued", "started", "running"])

const loopStageRow = (intent: IntentRow): Html =>
  h.ol(
    [cls("loop-stages")],
    loopStageStates(intent).map((stage) =>
      h.li(
        [cls(`loop-stage loop-stage-${stage.state}`)],
        [
          h.span([cls("loop-stage-dot")], []),
          h.span([cls("loop-stage-label")], [stage.label]),
        ],
      ),
    ),
  )

const intentCard = (intent: IntentRow): Html => {
  const sl = shipStatusLine(intent.status)
  const gate = shipGateLine(intent.status)
  return h.section(
    [cls("loop-intent"), h.DataAttribute("autopilot-intent-id", intent.intentId)],
    [
      h.div(
        [cls("loop-intent-head")],
        [
          h.span([cls("loop-intent-title")], [intentLabel(intent)]),
          h.span(
            [cls("loop-intent-status"), h.Style({ color: sl.dotColor })],
            [sl.text],
          ),
        ],
      ),
      loopStageRow(intent),
      h.p([cls(`loop-ship-gate loop-ship-${gate.tone}`)], [gate.text]),
    ],
  )
}

const intentQueue = (intents: ReadonlyArray<IntentRow>): Html =>
  h.section(
    [cls("card")],
    [
      h.h2([cls("card-title")], ["Intent queue"]),
      intents.length === 0
        ? h.p([cls("empty-state")], ["No asks yet. Submit one from the Ask card."])
        : h.div([cls("loop-intents")], intents.map(intentCard)),
    ],
  )

// The fan-out work pool: the node's active coding sessions (the agents the
// coordinator fans intents into). A read projection over `session.list`; each
// row links to session-detail. We show only non-terminal sessions to keep the
// loop view focused on what is running now.
const fanoutSessions = (sessions: ReadonlyArray<SessionSummary>): Html => {
  const active = sessions.filter((session) => ACTIVE_SESSION_STATES.has(session.state))
  return h.section(
    [cls("card")],
    [
      h.h2([cls("card-title")], ["Fan-out — active sessions"]),
      active.length === 0
        ? h.p([cls("empty-state")], ["No sessions running."])
        : h.div(
            [cls("loop-sessions")],
            active.map((session) =>
              h.div(
                [
                  cls("loop-session-row"),
                  h.Tabindex(0),
                  h.DataAttribute("autopilot-session-ref", session.sessionRef),
                  h.OnClick(SelectedSession({ sessionRef: session.sessionRef })),
                ],
                [
                  h.span([cls("loop-session-adapter")], [session.adapter]),
                  h.span([cls("loop-session-ref")], [session.sessionRef.slice(-12)]),
                  h.span([cls("loop-session-state")], [session.state]),
                ],
              ),
            ),
          ),
    ],
  )
}

const loopHeader = (model: Model): Html => {
  const node = modelNode(model)
  const intents: ReadonlyArray<IntentRow> = node?.intents ?? []
  const paused = node?.coordinatorPaused ?? null
  const toggle =
    paused === null
      ? h.empty
      : h.button(
          [
            cls(`coord-toggle ${paused ? "coord-paused" : ""}`),
            h.Type("button"),
            h.OnClick(ClickedCoordinatorToggle({ paused: !paused })),
          ],
          [coordinatorToggleLabel(paused)],
        )
  return h.div(
    [cls("loop-header")],
    [
      h.h1([cls("pane-title")], ["Autonomous loop"]),
      h.div(
        [cls("loop-header-meta")],
        [h.p([cls("node-status")], [autonomousLoopSummary(intents, paused)]), toggle],
      ),
    ],
  )
}

export const autonomousLoopPane = (model: Model): Html => {
  const node = modelNode(model)
  const intents: ReadonlyArray<IntentRow> = node?.intents ?? []
  const sessions: ReadonlyArray<SessionSummary> = node?.sessions ?? []
  return h.div(
    [cls("loop-pane")],
    [
      loopHeader(model),
      h.p(
        [cls("card-body loop-explainer")],
        [
          "The coordinator turns each ask into a plan, fans it across agents, " +
            "reconciles their work, then reaches the ship gate. The ship gate " +
            "defaults to DENY, so an autonomous run escalates to you rather than " +
            "spending. This view is read-only.",
        ],
      ),
      intentQueue(intents),
      fanoutSessions(sessions),
    ],
  )
}
