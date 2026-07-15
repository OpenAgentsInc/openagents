import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { bundledCodexExecutableSha256 } from "@openagentsinc/codex-app-server-protocol/compatibility"
import { createCodexAppServerSupervisor } from "../src/codex-app-server-supervisor.ts"
import { openCodexDurableQueue } from "../src/codex-durable-queue.ts"
import { makeCodexTurnState } from "../src/codex-turn-state.ts"

const binary = process.env.CODEX_BIN
if (!binary) throw new Error("CODEX_BIN must name the exact packaged Codex executable")
const root = mkdtempSync(join(tmpdir(), "openagents-codex-turn-control-"))
const supervisor = createCodexAppServerSupervisor({ nativeJournalRoot: join(root, "native"), strictGeneratedDecoding: true })
const queue = openCodexDurableQueue(join(root, "queue.json"))
try {
  const env = { ...process.env }; delete env.CODEX_HOME
  const lease = await supervisor.acquire({ binary, binarySha256: bundledCodexExecutableSha256, env, cwd: root, accountRef: "codex-current", hostTarget: "local-desktop-smoke", requestTimeoutMs: 120_000 })
  const state = makeCodexTurnState({ receiptPath: join(root, "steer-receipts.json") })
  const terminal = new Map<string, () => void>()
  const remove = lease.subscribe(notification => {
    state.apply(notification)
    const params = notification.message.params as { turn?: { id?: string } } | undefined
    if (notification.message.method === "turn/completed" && params?.turn?.id) terminal.get(params.turn.id)?.()
  })
  const waitTerminal = (turnId: string) => state.snapshot().terminalTurnIds.includes(turnId) ? Promise.resolve() : new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`turn ${turnId} did not terminalize`)), 120_000)
    terminal.set(turnId, () => { clearTimeout(timer); terminal.delete(turnId); resolve() })
  })
  const interruptIfActive = async (interruptThreadId: string, turnId: string) => {
    if (state.snapshot().terminalTurnIds.includes(turnId)) return
    try { await lease.request("turn/interrupt", { threadId: interruptThreadId, turnId }) }
    catch (error) {
      if (!state.snapshot().terminalTurnIds.includes(turnId) && !(error instanceof Error && error.message.includes("no active turn"))) throw error
    }
  }
  const started = await lease.request("thread/start", { model: "gpt-5.6-sol", cwd: root, approvalPolicy: "never", sandbox: "read-only", ephemeral: false, threadSource: "appServer" }) as { thread?: { id?: string } }
  const threadId = started.thread?.id
  if (!threadId) throw new Error("thread/start omitted id")
  const first = await lease.request("turn/start", { threadId, clientUserMessageId: "smoke-user-1", input: [{ type: "text", text: "Do not use tools. Reply with exactly ORIGINAL.", text_elements: [] }], cwd: root, model: "gpt-5.6-sol", effort: "low", approvalPolicy: "never", sandboxPolicy: { type: "readOnly", networkAccess: false } }) as { turn?: { id?: string } }
  const firstTurnId = first.turn?.id
  if (!firstTurnId) throw new Error("turn/start omitted id")
  state.bindStartedTurn(threadId, firstTurnId)
  const steer = state.authorizeSteer(threadId, firstTurnId, "smoke-steer-1")
  if (!steer.accepted) throw new Error("active regular turn rejected exact steer")
  try {
    await lease.request("turn/steer", { threadId, expectedTurnId: firstTurnId, clientUserMessageId: steer.clientUserMessageId, input: [{ type: "text", text: "Reply with exactly STEERED.", text_elements: [] }] })
    state.settleSteer(threadId, firstTurnId, steer.clientUserMessageId, true)
  } catch (error) {
    state.settleSteer(threadId, firstTurnId, steer.clientUserMessageId, false)
    throw error
  }
  await waitTerminal(firstTurnId)
  const queued = queue.enqueue(threadId, "Do not use tools. Reply with exactly QUEUED.")
  const claimed = queue.claimNext(threadId, `${firstTurnId}:completed`)
  if (claimed?.queueRef !== queued.queueRef) throw new Error("durable queue did not promote first intent")
  queue.admitPromotion(claimed.queueRef, threadId, claimed.clientUserMessageId)
  const second = await lease.request("turn/start", { threadId, clientUserMessageId: claimed.clientUserMessageId, input: [{ type: "text", text: claimed.message, text_elements: [] }], cwd: root, model: "gpt-5.6-sol", effort: "low", approvalPolicy: "never", sandboxPolicy: { type: "readOnly", networkAccess: false } }) as { turn?: { id?: string } }
  const secondTurnId = second.turn?.id
  if (!secondTurnId) throw new Error("queued turn/start omitted id")
  state.bindStartedTurn(threadId, secondTurnId)
  if (!state.admitInterrupt(threadId, secondTurnId)) throw new Error("interrupt was not admitted")
  await interruptIfActive(threadId, secondTurnId)
  if (!state.snapshot().terminalTurnIds.includes(secondTurnId) && state.snapshot().activeTurnId !== secondTurnId) throw new Error("interrupt ACK incorrectly terminalized turn")
  await waitTerminal(secondTurnId)
  queue.complete(claimed.queueRef, secondTurnId)
  const inline = await state.startReview(lease, { threadId, delivery: "inline", target: { type: "custom", instructions: "Return no findings and do not use tools." } })
  if (!inline.turnId) throw new Error("inline review omitted turn id")
  await interruptIfActive(threadId, inline.turnId)
  await waitTerminal(inline.turnId)
  const detached = await state.startReview(lease, { threadId, delivery: "detached", target: { type: "custom", instructions: "Return no findings and do not use tools." } })
  if (!detached.turnId || !detached.reviewThreadId || detached.reviewThreadId === threadId) throw new Error("detached review identities were not distinct")
  await interruptIfActive(detached.reviewThreadId, detached.turnId)
  await waitTerminal(detached.turnId)
  remove(); lease.release()
  console.log(`Verified exact steer, terminal interrupt, durable queued turn, and inline/detached review on ${threadId}.`)
} finally {
  queue.close(); supervisor.close(); rmSync(root, { recursive: true, force: true })
}
