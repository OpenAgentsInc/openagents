// CS-A1: provider/account picker + multi-account management.
//
// Covers (a) the Bun-side dev.accounts config CRUD (add/remove/set-priority/
// list), and (b) the pure reducer for the composer per-session account picker,
// the apple_fm spawn-adapter routing, and the management messages — all on the
// EXISTING control protocol (session.spawn accountRef / apple_fm.session.start
// / accounts.list); no new control verb.

import { describe, expect, test } from "bun:test"
import { mkdtempSync, mkdirSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  accountConfigPath,
  addManagedAccount,
  listManagedAccounts,
  removeManagedAccount,
  setManagedAccountPriority,
} from "../src/bun/account-management"
import { initialModel, Model, modelManagedAccounts, modelPaneLayer } from "../src/ui/model"
import type { NodeStateMessage } from "../src/shared/rpc"
import {
  ChangedAddAccountHome,
  ChangedAddAccountRef,
  ChangedVerseMode,
  ClickedAddManagedAccount,
  ClickedBumpManagedAccountPriority,
  ClickedComposerSpawn,
  GotNodeState,
  GotManagedAccounts,
  OpenedManagedPane,
  SelectedComposerAccount,
  SettledManagedAccountMutation,
} from "../src/ui/message"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

const serializeView = (node: unknown): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(node, (_key, value) => {
    if (typeof value === "function") return "[fn]"
    if (typeof value === "object" && value !== null) {
      if (seen.has(value)) return "[cycle]"
      seen.add(value)
    }
    return value
  })
}

const withHome = (fn: (home: string) => void) => {
  const home = mkdtempSync(join(tmpdir(), "cs-a1-accounts-"))
  try {
    fn(home)
  } finally {
    rmSync(home, { recursive: true, force: true })
  }
}

describe("CS-A1 account-management config CRUD", () => {
  test("add → list round-trips a new dev.accounts entry", () => {
    withHome((home) => {
      const accountHome = join(home, ".codex-work")
      mkdirSync(accountHome, { recursive: true })
      const added = addManagedAccount(home, {
        ref: "work",
        provider: "codex",
        home: accountHome,
        priority: 1,
      })
      expect(added.ok).toBe(true)
      const listed = listManagedAccounts(home)
      expect(listed.ok).toBe(true)
      expect(listed.accounts).toHaveLength(1)
      expect(listed.accounts[0]?.ref).toBe("work")
      expect(listed.accounts[0]?.provider).toBe("codex")
      expect(listed.accounts[0]?.priority).toBe(1)
      expect(listed.accounts[0]?.homePresent).toBe(true)
      // The runtime reads this exact file/shape (loadPylonAccountRegistry).
      const raw = JSON.parse(readFileSync(accountConfigPath(home), "utf8"))
      expect(raw.dev.accounts[0].ref).toBe("work")
      expect(raw.dev.accounts[0].home).toBe(accountHome)
      expect(raw.dev.accounts[0].priority).toBe(1)
    })
  })

  test("add rejects a duplicate ref for the same provider", () => {
    withHome((home) => {
      addManagedAccount(home, { ref: "a", provider: "codex", home })
      const dup = addManagedAccount(home, { ref: "a", provider: "codex", home })
      expect(dup.ok).toBe(false)
      expect(dup.error).toContain("already exists")
    })
  })

  test("add rejects an invalid ref", () => {
    withHome((home) => {
      const bad = addManagedAccount(home, { ref: "has space", provider: "codex", home })
      expect(bad.ok).toBe(false)
    })
  })

  test("set-priority reorders the list (lower runs first)", () => {
    withHome((home) => {
      addManagedAccount(home, { ref: "a", provider: "codex", home, priority: 5 })
      addManagedAccount(home, { ref: "b", provider: "codex", home, priority: 5 })
      setManagedAccountPriority(home, { ref: "b", provider: "codex", priority: 1 })
      const listed = listManagedAccounts(home)
      expect(listed.accounts.map((r) => r.ref)).toEqual(["b", "a"])
      expect(listed.accounts[0]?.priority).toBe(1)
    })
  })

  test("Codex accounts sort before Claude Agent rows at equal priority", () => {
    withHome((home) => {
      addManagedAccount(home, { ref: "z-claude", provider: "claude_agent", home, priority: 3 })
      addManagedAccount(home, { ref: "a-codex", provider: "codex", home, priority: 3 })
      const listed = listManagedAccounts(home)
      expect(listed.accounts.map((r) => `${r.provider}:${r.ref}`)).toEqual([
        "codex:a-codex",
        "claude_agent:z-claude",
      ])
    })
  })

  test("remove deletes the entry", () => {
    withHome((home) => {
      addManagedAccount(home, { ref: "a", provider: "claude_agent", home })
      const removed = removeManagedAccount(home, { ref: "a", provider: "claude_agent" })
      expect(removed.ok).toBe(true)
      expect(removed.accounts).toHaveLength(0)
    })
  })

  test("set-priority on an unknown ref fails", () => {
    withHome((home) => {
      const res = setManagedAccountPriority(home, {
        ref: "nope",
        provider: "codex",
        priority: 1,
      })
      expect(res.ok).toBe(false)
    })
  })

  test("write preserves unrelated config keys", () => {
    withHome((home) => {
      const configPath = accountConfigPath(home)
      mkdirSync(home, { recursive: true })
      require("node:fs").writeFileSync(
        configPath,
        JSON.stringify({ keepMe: "value", dev: { accounts: [] } }),
      )
      addManagedAccount(home, { ref: "a", provider: "codex", home })
      const raw = JSON.parse(readFileSync(configPath, "utf8"))
      expect(raw.keepMe).toBe("value")
      expect(raw.dev.accounts).toHaveLength(1)
    })
  })

  test("missing node home is reported, not thrown", () => {
    expect(listManagedAccounts(null).ok).toBe(false)
    expect(addManagedAccount(null, { ref: "a", provider: "codex", home: "/x" }).ok).toBe(false)
  })
})

const nodeWithAccounts = (
  accounts: NodeStateMessage["accounts"],
): NodeStateMessage => ({
  ok: true,
  schema: "openagents.pylon.control.v0.3",
  sessions: [],
  accounts,
})

describe("CS-A1 composer account picker reducer", () => {
  test("SelectedComposerAccount sets/clears the per-session account", () => {
    const [picked] = update(initialModel, SelectedComposerAccount({ accountRef: "work" }))
    expect(picked.composerAccountRef).toBe("work")
    const [cleared] = update(picked, SelectedComposerAccount({ accountRef: null }))
    expect(cleared.composerAccountRef).toBe(null)
  })

  test("ClickedComposerSpawn threads the selected accountRef through the spawn", () => {
    const start = Model.make({
      ...initialModel,
      spawnAdapter: "codex",
      spawnObjective: "add a /health route",
      composerAccountRef: "work",
    })
    const [model, commands] = update(start, ClickedComposerSpawn())
    expect(model.composerPending).toBe(true)
    expect(commands).toHaveLength(1)
    const cmd = commands[0] as unknown as { args?: { accountRef?: string | null } }
    expect(cmd.args?.accountRef).toBe("work")
  })

  test("ClickedComposerSpawn with apple_fm routes through the Apple FM command", () => {
    const start = Model.make({
      ...initialModel,
      spawnAdapter: "apple_fm",
      spawnObjective: "summarize the workspace",
    })
    const [model, commands] = update(start, ClickedComposerSpawn())
    expect(model.composerPending).toBe(true)
    expect(model.composerTurns).toEqual(["summarize the workspace"])
    expect(commands).toHaveLength(1)
    // The apple_fm command carries the objective + worktree, no lane/account.
    const cmd = commands[0] as unknown as { args?: { objective?: string; accountRef?: unknown } }
    expect(cmd.args?.objective).toBe("summarize the workspace")
    expect(cmd.args?.accountRef).toBeUndefined()
  })

  test("apple_fm spawn validates a non-empty objective", () => {
    const start = Model.make({
      ...initialModel,
      spawnAdapter: "apple_fm",
      spawnObjective: "   ",
    })
    const [model, commands] = update(start, ClickedComposerSpawn())
    expect(model.composerStatus.tone).toBe("error")
    expect(commands).toHaveLength(0)
  })
})

describe("CS-A1 account-management reducer", () => {
  test("entering Verse code mode requests the managed Codex account inventory", () => {
    const start = Model.make({ ...initialModel, pane: "chat" })
    const [model, commands] = update(start, ChangedVerseMode({ mode: "code" }))
    expect(model.verseMode).toBe("code")
    expect(model.managedAccountsPending).toBe(true)
    expect(model.managedAccountsStatus.text).toContain("Codex accounts")
    expect(commands.map((command) => command.name)).toEqual(["LoadManagedAccounts"])
  })

  test("GotManagedAccounts stores the projection and clears pending", () => {
    const start = Model.make({ ...initialModel, managedAccountsPending: true })
    const [model] = update(
      start,
      GotManagedAccounts({
        projection: { ok: true, accounts: [{ ref: "work", provider: "codex", homePresent: true, priority: 1 }] },
      }),
    )
    expect(model.managedAccountsPending).toBe(false)
    expect(modelManagedAccounts(model)?.accounts).toHaveLength(1)
  })

  test("ClickedAddManagedAccount requires ref + home", () => {
    const [model, commands] = update(initialModel, ClickedAddManagedAccount())
    expect(model.managedAccountsStatus.tone).toBe("error")
    expect(commands).toHaveLength(0)
  })

  test("ClickedAddManagedAccount rejects invalid refs before dispatch", () => {
    let [model] = update(initialModel, ChangedAddAccountRef({ value: "has space" }))
    ;[model] = update(model, ChangedAddAccountHome({ value: "~/.codex-work" }))
    const [next, commands] = update(model, ClickedAddManagedAccount())
    expect(next.managedAccountsStatus.tone).toBe("error")
    expect(next.managedAccountsStatus.text).toContain("invalid")
    expect(commands).toHaveLength(0)
  })

  test("ClickedAddManagedAccount rejects duplicate provider refs before dispatch", () => {
    const start = Model.make({
      ...initialModel,
      addAccountRef: "work",
      addAccountHome: "~/.codex-work",
      managedAccounts: {
        ok: true,
        accounts: [
          { ref: "work", provider: "codex", homePresent: true, priority: 1 },
        ],
      },
    })
    const [model, commands] = update(start, ClickedAddManagedAccount())
    expect(model.managedAccountsStatus.tone).toBe("error")
    expect(model.managedAccountsStatus.text).toContain("already exists")
    expect(commands).toHaveLength(0)
  })

  test("ClickedAddManagedAccount dispatches the add command when valid", () => {
    let [model] = update(initialModel, ChangedAddAccountRef({ value: "work" }))
    ;[model] = update(model, ChangedAddAccountHome({ value: "~/.codex-work" }))
    const [next, commands] = update(model, ClickedAddManagedAccount())
    expect(next.managedAccountsPending).toBe(true)
    expect(commands).toHaveLength(1)
  })

  test("SettledManagedAccountMutation success clears the add form + stores list", () => {
    const start = Model.make({
      ...initialModel,
      addAccountRef: "work",
      addAccountHome: "~/.codex-work",
      managedAccountsPending: true,
    })
    const [model] = update(
      start,
      SettledManagedAccountMutation({
        projection: { ok: true, accounts: [{ ref: "work", provider: "codex", homePresent: true, priority: null }] },
      }),
    )
    expect(model.managedAccountsStatus.tone).toBe("success")
    expect(model.addAccountRef).toBe("")
    expect(model.addAccountHome).toBe("")
    expect(modelManagedAccounts(model)?.accounts).toHaveLength(1)
  })

  test("ClickedBumpManagedAccountPriority dispatches a set-priority command", () => {
    const [model, commands] = update(
      initialModel,
      ClickedBumpManagedAccountPriority({ ref: "work", provider: "codex", priority: 0 }),
    )
    expect(model.managedAccountsPending).toBe(true)
    expect(commands).toHaveLength(1)
  })

  test("Verse code mode opens a dedicated Accounts pane with live refresh commands", () => {
    const start = Model.make({
      ...initialModel,
      pane: "chat",
      verseMode: "code",
      managedAccounts: {
        ok: true,
        accounts: [
          { ref: "work", provider: "codex", homePresent: false, priority: 1 },
          { ref: "claude-lab", provider: "claude_agent", homePresent: true, priority: 1 },
        ],
      },
    })
    const [model, commands] = update(start, OpenedManagedPane({ pane: "accounts" }))
    expect(model.pane).toBe("chat")
    expect(model.verseMode).toBe("code")
    expect(modelPaneLayer(model).panes.map((pane) => pane.kind)).toEqual(["accounts"])
    expect(commands.map((command) => command.name)).toEqual([
      "LoadManagedAccounts",
      "LoadInferenceGatewayReadiness",
    ])

    const tree = serializeView(view(model).body)
    expect(tree).toContain("pane-window")
    expect(tree).toContain("Accounts")
    expect(tree).toContain("Add account")
    expect(tree).toContain("work")
    expect(tree).toContain("home missing")
    expect(tree).toContain("claude_agent")
  })

  test("managed account mutations update Verse code inventory without restarting Verse", () => {
    const start = Model.make({
      ...initialModel,
      pane: "chat",
      verseMode: "code",
      managedAccounts: {
        ok: true,
        accounts: [
          { ref: "work", provider: "codex", homePresent: true, priority: 3 },
        ],
      },
    })
    let tree = serializeView(view(start).body)
    expect(tree).toContain("work")
    expect(tree).toContain("prio 3")
    expect(tree).not.toContain("personal")

    const [reprioritized] = update(
      start,
      SettledManagedAccountMutation({
        projection: {
          ok: true,
          accounts: [
            { ref: "work", provider: "codex", homePresent: true, priority: 1 },
            { ref: "personal", provider: "codex", homePresent: true, priority: 2 },
          ],
        },
      }),
    )
    expect(reprioritized.pane).toBe("chat")
    expect(reprioritized.verseMode).toBe("code")
    tree = serializeView(view(reprioritized).body)
    expect(tree).toContain("work")
    expect(tree).toContain("personal")
    expect(tree).toContain("prio 1")
    expect(tree).toContain("prio 2")

    const [removed] = update(
      reprioritized,
      SettledManagedAccountMutation({
        projection: {
          ok: true,
          accounts: [
            { ref: "personal", provider: "codex", homePresent: true, priority: 2 },
          ],
        },
      }),
    )
    expect(removed.pane).toBe("chat")
    expect(removed.verseMode).toBe("code")
    tree = serializeView(view(removed).body)
    expect(tree).toContain("personal")
    expect(tree).not.toContain("work")
  })
})

// A live node-state projection drives the picker view's account list; assert the
// AccountRow shape carries the picker fields so the view can render/select them.
describe("CS-A1 node-state account rows feed the picker", () => {
  test("accounts carry accountRef/selector for the per-session picker", () => {
    const node = nodeWithAccounts([
      {
        provider: "codex",
        homeState: "present",
        ready: true,
        accountRef: "work",
        accountRefHash: "account.pylon.codex.abc",
        selector: "registry_ref",
        blockerRefs: [],
        priority: null,
      },
    ])
    expect(node.accounts?.[0]?.accountRef).toBe("work")
    expect(node.accounts?.[0]?.selector).toBe("registry_ref")
  })

  test("Verse code mode renders concise Codex account inventory without full hashes", () => {
    const node = nodeWithAccounts([
      {
        provider: "codex",
        homeState: "present",
        ready: true,
        accountRef: "work",
        accountRefHash: "account.pylon.codex.workabcdef",
        selector: "registry_ref",
        blockerRefs: [],
        priority: 2,
      },
      {
        provider: "codex",
        homeState: "present",
        ready: false,
        accountRef: "personal",
        accountRefHash: "account.pylon.codex.personaldef456",
        selector: "registry_ref",
        blockerRefs: ["codex.login_required"],
        priority: 3,
      },
    ])
    let model = Model.make({
      ...initialModel,
      pane: "chat",
      verseMode: "code",
      composerAccountRef: "personal",
    })
    ;[model] = update(
      model,
      GotManagedAccounts({
        projection: {
          ok: true,
          accounts: [
            { ref: "work", provider: "codex", homePresent: true, priority: 2 },
            { ref: "personal", provider: "codex", homePresent: true, priority: 3 },
          ],
        },
      }),
    )
    ;[model] = update(model, GotNodeState({ node }))

    const tree = serializeView(view(model).body)
    expect(tree).toContain("verse-code-account-inventory")
    expect(tree).toContain("Codex accounts")
    expect(tree).toContain("work")
    expect(tree).toContain("personal")
    expect(tree).toContain("using personal")
    expect(tree).toContain("ready")
    expect(tree).toContain("blocked")
    expect(tree).toContain("prio 2")
    expect(tree).toContain("prio 3")
    expect(tree).toContain("registry")
    expect(tree).toContain("#abcdef")
    expect(tree).toContain("#def456")
    expect(tree).not.toContain("account.pylon.codex.workabcdef")
    expect(tree).not.toContain("account.pylon.codex.personaldef456")
    expect(tree).not.toContain("codex.login_required")
  })

  test("Verse explore mode does not render the Codex account inventory", () => {
    const model = Model.make({
      ...initialModel,
      pane: "chat",
      verseMode: "explore",
      managedAccounts: {
        ok: true,
        accounts: [
          { ref: "work", provider: "codex", homePresent: true, priority: 1 },
        ],
      },
    })
    const tree = serializeView(view(model).body)
    expect(tree).not.toContain("verse-code-account-inventory")
    expect(tree).not.toContain("Codex accounts")
  })
})
