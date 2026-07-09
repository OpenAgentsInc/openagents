/**
 * P0.8 (#8543) unattended straight-line E2E — typed leg registry.
 *
 * The owner-approved seeded public-safe account exists: GitHub user
 * `AgentFlampy` with the fork `AgentFlampy/openagents` (recorded on #8543,
 * 2026-07-09). This module is the single typed statement of WHICH legs of the
 * launch straight line ("sign in → grant visible → pick repo → dispatch turn
 * → live updates → push/writeback link → credits drain") are genuinely
 * runnable unattended today, and which are BLOCKED with a named blocker —
 * so the runner (`scripts/straight-line-e2e-run.sh`) can only ever record a
 * blocked leg as a typed skip, never as a fake pass.
 *
 * Guarded by tests/straight-line-e2e.test.ts.
 */

export const KhalaMobileStraightLineE2eSchemaId =
  "openagents.khala_mobile.straight_line_e2e_receipt.v1" as const

/** The #8543 owner-approved seed identity (public-safe; the credential
 * itself lives only in the gitignored `~/work/.secrets/khala-maestro.env`). */
export const STRAIGHT_LINE_SEED_ACCOUNT = "AgentFlampy" as const
export const STRAIGHT_LINE_SEED_REPO = "AgentFlampy/openagents" as const

export type StraightLineLegMode = "runnable" | "blocked"

export type StraightLineLeg = Readonly<{
  id: string
  title: string
  mode: StraightLineLegMode
  /** Maestro flow (for runnable legs) or "" when no flow may run yet. */
  flow: string
  /** Non-empty for blocked legs: the typed reason no run may be recorded. */
  blockerRefs: readonly string[]
  notes: string
}>

export const STRAIGHT_LINE_LEGS: readonly StraightLineLeg[] = [
  {
    blockerRefs: [],
    flow: "SignedInThreadSmoke.yaml",
    id: "ios_signed_in_thread_smoke",
    mode: "runnable",
    notes:
      "Sign-in resolves from the seeded AgentFlampy build, the seeded thread opens, the composer lane picker is visible, a typed message sends and renders.",
    title: "Signed-in thread smoke (iOS simulator)",
  },
  {
    blockerRefs: [],
    flow: "RepoPickerReachable.yaml",
    id: "ios_repo_picker_reachable",
    mode: "runnable",
    notes:
      "The thread's repo chip opens the real RepoPickerScreen (header + search chrome). Asserts nothing about list contents — those are mobile-USER-session-gated.",
    title: "Repo picker reachable from the thread (iOS simulator)",
  },
  {
    blockerRefs: [],
    flow: "SignedInThreadReply.yaml",
    id: "ios_dispatch_reply",
    mode: "runnable",
    notes:
      "Send a deterministic prompt on the default hosted_khala lane and bounded-wait for the assistant reply token on-screen — dispatch + live updates, not just the sent bubble.",
    title: "Dispatch turn → live assistant reply (iOS simulator)",
  },
  {
    blockerRefs: [
      "blocker.khala_mobile.repo_list_requires_github_backed_mobile_session",
    ],
    flow: "StraightLineRepoPick.yaml",
    id: "ios_repo_pick_fork_bind",
    mode: "blocked",
    notes:
      "GET /api/mobile/repos is mobile-OpenAuth-USER-session-only by documented invariant (the seeded agent token 401s it BY DESIGN — see docs/khala-code/receipts/2026-07-07-qam-4-populated-happy-path.md). The runner probes the route live and runs this fail-closed flow (which targets the seeded fork AgentFlampy/openagents) the moment a captured real AgentFlampy mobile session exists in ~/work/.secrets/khala-mobile-session.env; until then it records a typed BLOCKED skip.",
    title: "Pick the seeded fork and bind it to the thread",
  },
  {
    blockerRefs: ["blocker.cx3.in_vm_cloud_execution_lane_missing.openagents#8547"],
    flow: "",
    id: "push_writeback",
    mode: "blocked",
    notes:
      "The pick-repo→push→writeback leg's cloud-execution half depends on CX-3's in-VM Codex lane (#8547, the rootfs/bake-host wall). No flow may run or be enforced over this leg until that lane exists.",
    title: "Push / writeback link from a dispatched cloud turn",
  },
  {
    blockerRefs: [
      "blocker.khala_mobile.credits_routes_require_github_backed_mobile_session",
    ],
    flow: "",
    id: "credits_grant_visible_drain",
    mode: "blocked",
    notes:
      "GET /api/mobile/credits/balance is mobile-USER-session-only (same gate as the repo list). The grant-visible half becomes provable via the QAM-4 populated visual lane once a session exists; the drain assertion additionally needs a flow that reads the balance before/after a dispatched turn.",
    title: "$10 grant visible and credits drain after a turn",
  },
] as const

export const runnableStraightLineLegIds = (): readonly string[] =>
  STRAIGHT_LINE_LEGS.filter(leg => leg.mode === "runnable").map(leg => leg.id)

export const blockedStraightLineLegIds = (): readonly string[] =>
  STRAIGHT_LINE_LEGS.filter(leg => leg.mode === "blocked").map(leg => leg.id)
