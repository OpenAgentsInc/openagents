// CS-A1 (#5361): end-to-end proof of the provider/account picker + multi-account
// management against a REAL loopback Pylon control server (a `pylon dev`-
// equivalent node), driven through the desktop's OWN Bun functions — the same
// path the webview uses:
//
//   - fetchNodeState() → the accounts.list projection the picker renders;
//   - spawnSession({ accountRef }) → a session spawned under a SELECTED account
//     (the per-session picker), proven by the session's accountRefHash;
//   - listManagedAccounts / addManagedAccount / setManagedAccountPriority /
//     removeManagedAccount → the account-management mutations round-trip through
//     the node's local dev.accounts config (the file the runtime reads).
//
// This is on the EXISTING control protocol (session.spawn already accepts
// accountRef #4868; accounts.list already exists). No connected paid provider
// account is required: the coding executor is a stub, and the account home is a
// scratch dir so the registry resolves it.
//
//   bun apps/autopilot-desktop/scripts/account-picker-proof.ts
//
// Output is a public-safe transcript (refs only) suitable for a launch update.

import { mkdtempSync } from "node:fs"
import { mkdir, writeFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"

import { createBootstrapSummary, parseBootstrapArgs } from "../../pylon/src/bootstrap"
import { collectPylonAccountsList } from "../../pylon/src/account-usage"
import { hashPylonAccountRef } from "../../pylon/src/account-registry"
import { startControlServer } from "../../pylon/src/node/control-server"
import {
  createControlSessionActions,
  type ControlSessionExecutor,
} from "../../pylon/src/node/control-sessions"
import { makePylonNodeRuntime } from "../../pylon/src/node/runtime"
import { PYLON_DEV_CHECK_SCHEMA } from "../../pylon/src/dev-loop"

import {
  addManagedAccount,
  listManagedAccounts,
  removeManagedAccount,
  setManagedAccountPriority,
} from "../src/bun/account-management"
import { fetchNodeState, spawnSession } from "../src/bun/pylon-control"

const TOKEN = "test-token-0123456789abcdef"

const fakeDevCheck = () => ({
  schema: PYLON_DEV_CHECK_SCHEMA,
  observedAt: "2026-06-18T00:00:00.000Z",
  action: "check" as const,
  state: "passed" as const,
  changeSummary: {
    repo: { state: "not_git" as const, rootRef: null, branch: null, commit: null },
    dirty: { state: "clean" as const, changedCount: 0, stagedCount: 0, unstagedCount: 0, untrackedCount: 0 },
    changedFileRefs: [],
    areaRefs: [],
    blockerRefs: [],
  },
  checkPlan: { state: "ready" as const, commandRefs: ["command.pylon.control_session.proof"], blockerRefs: [] },
  commandResults: [],
  latestRecordRef: null,
  branchUntouched: true,
  commitUntouched: true,
  pushPerformed: false,
  blockerRefs: [],
})

const log = (label: string, value?: unknown) =>
  value === undefined
    ? console.log(`\n=== ${label} ===`)
    : console.log(`  ${label}: ${typeof value === "string" ? value : JSON.stringify(value)}`)

async function main() {
  const root = mkdtempSync(join(tmpdir(), "account-picker-proof-"))
  const pylonHome = join(root, "pylon-home")
  const accountHomeWork = join(root, "codex-work")
  const accountHomePersonal = join(root, "codex-personal")
  const worktree = join(root, "worktree")
  const proofDir = join(root, "proofs")
  await mkdir(pylonHome, { recursive: true })
  await mkdir(accountHomeWork, { recursive: true })
  await mkdir(accountHomePersonal, { recursive: true })
  await mkdir(worktree, { recursive: true })

  // Seed the node config with one registry account so accounts.list and the
  // per-session accountRef resolution have something to select. The management
  // proof below adds/removes more through the desktop's Bun functions.
  await writeFile(
    join(pylonHome, "config.json"),
    `${JSON.stringify({ dev: { accounts: [{ ref: "work", provider: "codex", home: accountHomeWork }] } }, null, 2)}\n`,
  )
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })

  let ok = true
  try {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime

          // A stub executor — emits one composer/diff event like the real
          // codex/claude executors so the session reaches `completed`.
          const executor: ControlSessionExecutor = async (input) => {
            input.emit({ phase: "composer_event", message: "edited src/health.ts (+12 −0)", composerEventIndex: 1 })
            input.emit({ phase: "dev_check_started" })
            return {
              commandCount: 1,
              devCheck: fakeDevCheck(),
              editedFileCount: 1,
              eventCount: 2,
              externalSessionRef: "session.pylon.fake.external",
              responseDigestRef: "digest.pylon.fake.response",
              totalTokens: 21,
            }
          }

          const server = yield* startControlServer(runtime, {
            token: TOKEN,
            actions: {
              sessions: createControlSessionActions({
                executor,
                proofsDir: proofDir,
                summary,
              }),
              // CS-A1: the picker reads this projection. Same `accounts.list`
              // verb + collector the production node wires.
              accountsList: () => collectPylonAccountsList(summary, { env: { PYLON_ACCOUNT_HOME_ROOT: root } }),
            },
            port: 0,
          })
          const baseUrl = server.url
          log("Loopback control server up (pylon dev-equivalent)")
          log("control url", baseUrl)

          // 1) ACCOUNT LIST POPULATES — the projection the picker renders.
          log("STEP 1 — accounts.list populates (desktop fetchNodeState → picker source)")
          const node = yield* Effect.promise(() => fetchNodeState({ baseUrl, token: TOKEN }))
          const workAccount = node.accounts.find((a) => a.accountRef === "work")
          log("accounts surfaced", node.accounts.map((a) => `${a.provider}:${a.accountRef ?? "default"}`).join(", "))
          log("registry 'work' account present", workAccount !== undefined)
          log("work account ref hash", workAccount?.accountRefHash ?? "(none)")
          if (workAccount === undefined) throw new Error("registry 'work' account did not surface in accounts.list")
          if (workAccount.accountRef !== "work") throw new Error("account ref not surfaced for the picker")

          // 2) SPAWN UNDER A SELECTED ACCOUNT — the per-session picker path.
          log("STEP 2 — spawn under the selected account (session.spawn accountRef)")
          const expectedHash = hashPylonAccountRef("codex", "work")
          const spawn = yield* Effect.promise(() =>
            spawnSession({
              baseUrl,
              token: TOKEN,
              adapter: "codex",
              objective: "add a GET /health route and a test",
              verify: ["bun", "--version"],
              worktreePath: worktree,
              accountRef: "work",
            }),
          )
          if (!spawn.ok) throw new Error(`spawn failed: ${spawn.error}`)
          log("spawned sessionRef", spawn.sessionRef)

          // Poll until the session lands and assert it ran under the chosen account.
          let accountRefHash: string | null = null
          let state = ""
          for (let attempt = 0; attempt < 60; attempt += 1) {
            const live = yield* Effect.promise(() => fetchNodeState({ baseUrl, token: TOKEN }))
            const session = live.sessions.find((s) => s.sessionRef === spawn.sessionRef)
            state = session?.state ?? ""
            accountRefHash = session?.accountRefHash ?? null
            if (state === "completed" || state === "failed") break
            yield* Effect.sleep("25 millis")
          }
          log("session state", state)
          log("session accountRefHash", accountRefHash ?? "(none)")
          log("matches selected account hash", accountRefHash === expectedHash)
          if (accountRefHash !== expectedHash) {
            throw new Error("spawned session did not run under the selected account")
          }

          // 3) MANAGEMENT MUTATIONS ROUND-TRIP — add / set-priority / remove
          //    through the desktop's Bun functions, against the node's config.
          log("STEP 3 — account management mutations round-trip (dev.accounts config)")
          const added = addManagedAccount(pylonHome, {
            ref: "personal",
            provider: "codex",
            home: accountHomePersonal,
            priority: 2,
          })
          log("added 'personal'", added.ok)
          if (!added.ok) throw new Error(`add failed: ${added.error}`)

          const bumped = setManagedAccountPriority(pylonHome, {
            ref: "personal",
            provider: "codex",
            priority: 0,
          })
          log("set 'personal' priority=0", bumped.ok)
          const order = bumped.accounts.map((a) => `${a.ref}:${a.priority ?? "—"}`)
          log("registry order (priority asc)", order.join(", "))
          if (bumped.accounts[0]?.ref !== "personal") {
            throw new Error("priority change did not reorder the registry")
          }

          // The live accounts.list now reflects the new account too.
          const afterAdd = yield* Effect.promise(() => fetchNodeState({ baseUrl, token: TOKEN }))
          const personalLive = afterAdd.accounts.find((a) => a.accountRef === "personal")
          log("'personal' visible in accounts.list", personalLive !== undefined)
          if (personalLive === undefined) {
            throw new Error("added account did not appear in the live accounts.list projection")
          }

          const removed = removeManagedAccount(pylonHome, { ref: "personal", provider: "codex" })
          log("removed 'personal'", removed.ok)
          const finalList = listManagedAccounts(pylonHome)
          log("final managed refs", finalList.accounts.map((a) => a.ref).join(", "))
          if (finalList.accounts.some((a) => a.ref === "personal")) {
            throw new Error("remove did not delete the managed account")
          }

          log("PROOF COMPLETE — account picker + management drive the real control protocol end-to-end")
        }),
      ),
    )
  } catch (error) {
    ok = false
    console.error("\nPROOF FAILED:", error instanceof Error ? error.message : error)
  } finally {
    await rm(root, { recursive: true, force: true })
  }
  process.exit(ok ? 0 : 1)
}

void main()
