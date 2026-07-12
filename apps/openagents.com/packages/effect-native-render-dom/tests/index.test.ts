import { describe, expect, test } from "bun:test"
import { Window } from "../../../apps/web/node_modules/happy-dom"
import {
  Accordion,
  Button,
  CodeEditor,
  decodeCodeEditorHostProps,
  Icon,
  IconButton,
  IntentRef,
  List,
  SplitPane,
  Stack,
  Text,
  Transcript,
  UnknownIntentError,
  iconNames,
  type IntentReporter
} from "@effect-native/core"
import { Effect, Stream } from "@effect-native/core/effect"
import { Deferred } from "effect"

import { makeDomRenderer, makeStubCodeEditorDriver, scrollRegionIsAtEnd, scrollTopForPreservedAnchor, type DomHostContext, type DomHostDriver } from "../src/index"

test("transcript pinning distinguishes a reader scrolled up from the live edge", () => {
  expect(scrollRegionIsAtEnd({ scrollHeight: 1_000, scrollTop: 300, clientHeight: 400 })).toBe(false)
  expect(scrollRegionIsAtEnd({ scrollHeight: 1_000, scrollTop: 600, clientHeight: 400 })).toBe(true)
  expect(scrollRegionIsAtEnd({ scrollHeight: 1_000, scrollTop: 599, clientHeight: 400 })).toBe(true)
})

const nextTask = Effect.promise<void>(
  () => new Promise((resolve) => setTimeout(resolve, 0))
)

describe("DOM renderer host boundaries", () => {
  test("a changed Stack scroll target reveals once after scroll restoration", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    await Effect.runPromise(Effect.scoped(Effect.gen(function* () {
      const release = yield* Deferred.make<void>()
      const frame = (reveal: boolean) => Stack({
        key: "inspector-rail",
        direction: "column",
        ...(reveal ? { scrollToKey: "details-start" } : {}),
      }, [
        Text({ key: "agent-graph", content: "Tall graph", variant: "body" }),
        Text({ key: "details-start", content: "Message details", variant: "body" }),
      ])
      const stream = Stream.concat(
        Stream.succeed(frame(false)),
        Stream.fromEffect(Deferred.await(release).pipe(Effect.as(frame(true)))),
      )
      yield* makeDomRenderer({ document }).mount(root, stream, () => Effect.succeed(undefined))
      yield* nextTask
      const rail = root.querySelector<HTMLElement>('[data-en-key="inspector-rail"]')!
      const details = root.querySelector<HTMLElement>('[data-en-key="details-start"]')!
      Object.defineProperty(details, "offsetTop", { configurable: true, value: 640 })
      rail.scrollTop = 120
      yield* Deferred.succeed(release, undefined)
      yield* nextTask
      expect(rail.style.overflowY).toBe("auto")
      expect(rail.scrollTop).toBe(640)
      expect(rail.getAttribute("data-en-scroll-to-key")).toBe("details-start")
      expect(rail.hasAttribute("data-en-scroll-reveal-pending")).toBe(false)
      rail.scrollTop = 500
      yield* nextTask
      expect(rail.scrollTop).toBe(500)
    })))
  })

  test("compact icon buttons lower to a 32px icon-only accessible control", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    await Effect.runPromise(Effect.scoped(Effect.gen(function* () {
      yield* makeDomRenderer({ document }).mount(root, Stream.succeed(IconButton({
        key: "compact-add",
        icon: "Plus",
        size: "sm",
        accessibilityLabel: "Attach image",
        onPress: IntentRef("Attach"),
      })), () => Effect.succeed(undefined))
      const button = root.querySelector<HTMLButtonElement>('[data-en-key="compact-add"]')!
      expect(button.style.width).toBe("32px")
      expect(button.style.height).toBe("32px")
      expect(button.getAttribute("aria-label")).toBe("Attach image")
      expect(button.textContent).toBe("")
    })))
  })

  test("prepend anchoring corrects variable-height history before the next paint", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    await Effect.runPromise(Effect.scoped(Effect.gen(function* () {
      const release = yield* Deferred.make<void>()
      const frame = (older: boolean) => Stack({
        key: "anchored-scroll",
        direction: "column",
        preserveScrollAnchor: true,
      }, [
        ...(older ? [Text({ key: "older-row", content: "Older", variant: "body" })] : []),
        Text({ key: "visible-row", content: "Visible", variant: "body" }),
      ])
      const stream = Stream.concat(
        Stream.succeed(frame(false)),
        Stream.fromEffect(Deferred.await(release).pipe(Effect.as(frame(true)))),
      )
      yield* makeDomRenderer({ document }).mount(root, stream, () => Effect.succeed(undefined))
      yield* nextTask
      const container = root.querySelector<HTMLElement>('[data-en-key="anchored-scroll"]')!
      const anchor = root.querySelector<HTMLElement>('[data-en-key="visible-row"]')!
      Object.defineProperty(anchor, "offsetTop", { configurable: true, get: () =>
        container.querySelector('[data-en-key="older-row"]') === null ? 200 : 300 })
      Object.defineProperty(anchor, "offsetHeight", { configurable: true, value: 40 })
      container.scrollTop = 150
      yield* Deferred.succeed(release, undefined)
      yield* nextTask
      yield* nextTask
      expect(container.querySelector('[data-en-key="older-row"]')).not.toBeNull()
      expect(container.getAttribute("data-en-preserve-scroll-anchor")).toBe("true")
      expect((container.querySelector('[data-en-key="visible-row"]') as HTMLElement).offsetTop).toBe(300)
      expect(scrollTopForPreservedAnchor(50, 300)).toBe(250)
    })))
  })

  test("controlled accordion rerenders replace items instead of duplicating them", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const accordion = (expanded: boolean) => Accordion({
      key: "accounts-disclosure",
      items: [{ id: "accounts", header: "Accounts", content: [Text({ key: "account-row", content: "Codex", variant: "body" })] }],
      expandedIds: expanded ? ["accounts"] : [],
      onToggle: IntentRef("ToggleAccounts"),
    })
    await Effect.runPromise(Effect.scoped(Effect.gen(function* () {
      yield* makeDomRenderer({ document }).mount(root, Stream.make(accordion(false), accordion(true)), () => Effect.succeed(undefined))
      yield* nextTask
      expect(root.querySelectorAll('[data-en-accordion-item="accounts"]')).toHaveLength(1)
      expect(root.querySelector('[data-en-role="accordion-trigger"]')?.getAttribute("aria-expanded")).toBe("true")
      expect((root.querySelector('[data-en-role="accordion-content"]') as HTMLElement).hidden).toBe(false)
    })))
  })

  test("split pane drag resizes a fixed trailing inspector pane", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const reported: Array<unknown> = []
    const report: IntentReporter = (_ref, runtimeValue) => {
      reported.push(runtimeValue)
      return Effect.succeed(undefined)
    }
    await Effect.runPromise(Effect.scoped(Effect.gen(function* () {
      yield* makeDomRenderer({ document }).mount(root, Stream.succeed(SplitPane({
        key: "split-resizable",
        orientation: "row",
        onResize: IntentRef("ResizePane"),
        panes: [
          { id: "center", min: 360, content: Stack({ key: "center-resizable", direction: "column" }, []) },
          { id: "inspector", min: 280, max: 480, size: 336, content: Stack({ key: "inspector-resizable", direction: "column" }, []) },
        ],
      })), report)
      const divider = root.querySelector<HTMLElement>('[data-en-role="divider"]')!
      divider.dispatchEvent(new window.PointerEvent("pointerdown", { bubbles: true, clientX: 500, pointerId: 1 }))
      document.dispatchEvent(new window.PointerEvent("pointermove", { bubbles: true, clientX: 450, pointerId: 1 }))
      document.dispatchEvent(new window.PointerEvent("pointerup", { bubbles: true, clientX: 450, pointerId: 1 }))
      yield* nextTask
      expect(reported).toContainEqual({ paneId: "inspector", size: 386 })
    })))
  })

  test("split panes establish a bounded flex viewport for scrolling children", async () => {
    const window=new Window({url:"http://localhost/"});const document=window.document as unknown as Document;const root=document.createElement("div");document.body.appendChild(root)
    await Effect.runPromise(Effect.scoped(Effect.gen(function*(){yield* makeDomRenderer({document}).mount(root,Stream.succeed(SplitPane({key:"split",orientation:"row",panes:[{id:"center",content:Stack({key:"center",direction:"column",style:{flex:1,minHeight:0}},[])}]})),()=>Effect.succeed(undefined));const pane=root.querySelector<HTMLElement>('[data-en-pane="center"]')!;const content=root.querySelector<HTMLElement>('[data-en-key="center"]')!;expect(pane.style.display).toBe("flex");expect(pane.style.minHeight).toBe("0");expect(content.style.flex).not.toBe("") })))
  })
  test("lowers the closed semantic tree accessibility contract", async () => {
    const window = new Window({ url: "http://localhost/" }); const document = window.document as unknown as Document; const root = document.createElement("div"); document.body.appendChild(root)
    await Effect.runPromise(Effect.scoped(Effect.gen(function*(){ yield* makeDomRenderer({document}).mount(root,Stream.succeed(List({key:"agents",virtualize:false,a11y:{role:"tree",label:"Agents"}},[Button({key:"agent-child",label:"Worker · Running",variant:"ghost",onPress:IntentRef("SelectAgent"),a11y:{role:"treeitem",selected:true,expanded:false,level:2,positionInSet:1,setSize:2,tabIndex:0}}) as any])),()=>Effect.succeed(undefined)); const tree=root.querySelector('[data-en-key="agents"]'); const item=root.querySelector('[data-en-key="agent-child"]'); expect(tree?.getAttribute("role")).toBe("tree"); expect(item?.getAttribute("role")).toBe("treeitem"); expect(item?.getAttribute("aria-level")).toBe("2"); expect(item?.getAttribute("aria-posinset")).toBe("1"); expect(item?.getAttribute("aria-setsize")).toBe("2"); expect(item?.getAttribute("aria-selected")).toBe("true") })))
  })
  test("resolves every closed icon and lowers Compose through the ChatCompose asset", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const report: IntentReporter = () => Effect.succeed(undefined)

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          yield* makeDomRenderer({ document }).mount(
            root,
            Stream.succeed(Stack(
              { key: "icons", direction: "row" },
              iconNames.map((name) => Icon({ key: `icon-${name}`, name }))
            )),
            report
          )

          expect(root.querySelectorAll("[data-en-icon] svg")).toHaveLength(iconNames.length)
          const compose = root.querySelector('[data-en-icon="Compose"] svg')
          expect(compose?.outerHTML).toContain("M12 4.5")
        })
      )
    )
  })

  test("deduplicates atomic CSS while retaining the exact declaration value", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const view = Stack(
      {
        key: "outer",
        direction: "column",
        style: { backgroundColor: "accent" }
      },
      [
        Stack({
          key: "inner",
          direction: "column",
          style: { backgroundColor: "accent" }
        })
      ]
    )
    const report: IntentReporter = () => Effect.succeed(undefined)

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          const surface = yield* makeDomRenderer({ document }).mount(
            root,
            Stream.succeed(view),
            report
          )
          const outer = root.querySelector('[data-en-key="outer"]')
          const inner = root.querySelector('[data-en-key="inner"]')
          expect(outer?.className).toBe(inner?.className)

          const stylesheet = yield* surface.stylesheetText
          expect(stylesheet.match(/\.en-[a-z0-9]+\{background-color:var\(--en-color-accent\);\}/g)).toHaveLength(1)
        })
      )
    )
  })

  test("typed button styles override renderer defaults instead of leaving pill chrome inline", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const report: IntentReporter = () => Effect.succeed(undefined)

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          const surface = yield* makeDomRenderer({ document }).mount(
            root,
            Stream.succeed(Button({
              key: "plain-row",
              label: "Plain row title",
              variant: "ghost",
              onPress: IntentRef("OpenRow"),
              style: {
                padding: "0",
                borderWidth: 0,
                borderRadius: "none",
                color: "textPrimary",
                textAlign: "left"
              }
            })),
            report
          )

          const button = root.querySelector("button") as HTMLButtonElement
          expect(button.style.padding).toBe("")
          expect(button.style.border).toBe("")
          expect(button.style.borderRadius).toBe("")
          expect(button.style.color).toBe("")

          const stylesheet = yield* surface.stylesheetText
          expect(stylesheet).toContain("padding:var(--en-spacing-0)")
          expect(stylesheet).toContain("border-width:0")
          expect(stylesheet).toContain("border-radius:var(--en-radius-none)")
          expect(stylesheet).toContain("text-align:left")
        })
      )
    )
  })

  test("keeps a failing intent callback total while executing its effect", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const attempted: Array<string> = []
    const report: IntentReporter = (ref) =>
      Effect.sync(() => {
        attempted.push(ref.name)
      }).pipe(
        Effect.andThen(Effect.fail(new UnknownIntentError({ name: ref.name })))
      )

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          yield* makeDomRenderer({ document }).mount(
            root,
            Stream.succeed(Button({
              key: "failing-button",
              label: "Fail safely",
              variant: "primary",
              onPress: IntentRef("FailSafely")
            })),
            report
          )
          const button = root.querySelector("button") as HTMLButtonElement | null
          expect(button).not.toBeNull()
          expect(() => button?.click()).not.toThrow()
          yield* nextTask
          expect(attempted).toEqual(["FailSafely"])
        })
      )
    )
  })

  test("interrupts an in-flight intent when the mounted surface unmounts", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          const started = yield* Deferred.make<void>()
          const interrupted = yield* Deferred.make<void>()
          const report: IntentReporter = () =>
            Deferred.succeed(started, undefined).pipe(
              Effect.andThen(Effect.never),
              Effect.onInterrupt(() =>
                Effect.sleep("1 millis").pipe(
                  Effect.andThen(Deferred.succeed(interrupted, undefined))
                )
              )
            )
          const surface = yield* makeDomRenderer({ document }).mount(
            root,
            Stream.succeed(Button({
              key: "in-flight-button",
              label: "Start",
              variant: "primary",
              onPress: IntentRef("StartInFlight")
            })),
            report
          )
          const button = root.querySelector("button") as HTMLButtonElement | null
          expect(button).not.toBeNull()
          button?.click()
          yield* Deferred.await(started)

          yield* surface.unmount

          yield* Deferred.await(interrupted)
          expect(surface.root.isConnected).toBe(false)
        })
      )
    )
  })

  test("code-editor props carry only a bounded versioned selection command", () => {
    const view = CodeEditor({
      value: "const answer = 42",
      language: "typescript",
      selection: { start: 6, end: 12, version: 3 },
      fontScale: "body"
    })
    expect(view.kind).toBe("code-editor")
    expect(decodeCodeEditorHostProps(view.props)).toEqual({
      value: "const answer = 42",
      language: "typescript",
      selection: { start: 6, end: 12, version: 3 },
      fontScale: "body"
    })
    expect(() => decodeCodeEditorHostProps({
      value: "text",
      language: "plaintext",
      selection: { start: -1, end: 0, version: 0 }
    })).toThrow()
    expect(JSON.stringify(view)).not.toContain("monaco")
  })

  test("stub code-editor applies authoritative updates while focused and emits typed events", () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const emitted: Array<unknown> = []
    const driver = makeStubCodeEditorDriver()
    const initial = driver.decodeProps(CodeEditor({
      value: "initial",
      language: "typescript",
      selection: { start: 0, end: 3, version: 1 },
      wordWrap: false,
      minimap: false,
      fontScale: "body"
    }).props)
    const instance = driver.mount(root, initial, {
      document,
      report: () => Effect.succeed(undefined),
      emit: (payload) => emitted.push(payload)
    })
    const textarea = root.querySelector("textarea") as HTMLTextAreaElement
    textarea.focus()
    expect(textarea.selectionStart).toBe(0)
    expect(textarea.selectionEnd).toBe(3)

    textarea.value = "local"
    textarea.dispatchEvent(new window.Event("input"))
    instance.update(driver.decodeProps(CodeEditor({
      value: "authoritative",
      language: "json",
      selection: { start: 2, end: 8, version: 2 },
      wordWrap: true,
      minimap: true,
      fontScale: "caption"
    }).props))
    expect(textarea.value).toBe("authoritative")
    expect(textarea.selectionStart).toBe(2)
    expect(textarea.selectionEnd).toBe(8)
    expect(textarea.style.whiteSpace).toBe("pre-wrap")
    expect(textarea.style.fontSize).toContain("--en-type-caption-fontSize")
    expect(textarea.getAttribute("data-en-code-editor")).toBe("json")
    expect(textarea.getAttribute("data-en-minimap")).toBe("true")

    textarea.setSelectionRange(1, 4)
    textarea.dispatchEvent(new window.KeyboardEvent("keydown", { key: "s", metaKey: true }))
    expect(emitted).toEqual([
      { type: "change", value: "local" },
      { type: "selection", start: 1, end: 4 },
      { type: "save", value: "authoritative" }
    ])

    instance.unmount()
    instance.unmount()
    textarea.dispatchEvent(new window.Event("input"))
    expect(root.querySelector("textarea")).toBeNull()
    expect(emitted).toHaveLength(3)
  })

  test("replacement code-editor adapters update their event binding and dispose exactly once", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const reported: Array<string> = []
    const mounted: Array<unknown> = []
    const updated: Array<unknown> = []
    let unmounted = 0
    let context: DomHostContext | null = null
    const replacement: DomHostDriver = {
      kind: "code-editor",
      decodeProps: (props) => decodeCodeEditorHostProps(props),
      mount: (container, props, nextContext) => {
        mounted.push(props)
        context = nextContext
        const marker = nextContext.document.createElement("div")
        marker.setAttribute("data-replacement-editor", "true")
        container.appendChild(marker)
        return {
          update: (next) => {
            updated.push(next)
          },
          unmount: () => {
            unmounted += 1
            marker.remove()
          }
        }
      }
    }
    const report: IntentReporter = (ref) => Effect.sync(() => {
      reported.push(ref.name)
    })

    await Effect.runPromise(Effect.scoped(Effect.gen(function*() {
      const surface = yield* makeDomRenderer({ document, hostDrivers: [replacement] }).mount(
        root,
        Stream.make(
          CodeEditor({ key: "editor", value: "one", language: "text", onEvent: IntentRef("FirstEditorEvent") }),
          CodeEditor({ key: "editor", value: "two", language: "text", onEvent: IntentRef("LatestEditorEvent") })
        ),
        report
      )
      yield* nextTask
      expect(mounted).toHaveLength(1)
      expect(updated).toHaveLength(1)
      expect(root.querySelector('[data-replacement-editor="true"]')).not.toBeNull()
      context?.emit({ type: "change", value: "two" })
      yield* nextTask
      expect(reported).toEqual(["LatestEditorEvent"])
      yield* surface.unmount
      expect(unmounted).toBe(1)
    })))

    const removalRoot = document.createElement("div")
    document.body.appendChild(removalRoot)
    unmounted = 0
    await Effect.runPromise(Effect.scoped(Effect.gen(function*() {
      const surface = yield* makeDomRenderer({ document, hostDrivers: [replacement] }).mount(
        removalRoot,
        Stream.make(
          CodeEditor({ key: "removed-editor", value: "one", language: "text" }),
          Text({ key: "replacement-copy", content: "Editor closed", variant: "body" })
        ),
        report
      )
      yield* nextTask
      expect(unmounted).toBe(1)
      yield* surface.unmount
      expect(unmounted).toBe(1)
    })))
  })
})

// Commit idempotency guard (category-killer for the "details affordance flashes
// on every keystroke" bug). A transient-visibility affordance (opacity-0 at
// rest, revealed on :hover/:focus-within via a CSS transition) must NOT have
// that transition replayed by an unrelated re-render. The mechanism was that
// `replaceChildren` / freshly-created Transcript message wrappers re-parented
// the persisted keyed affordance on every commit; detaching + re-attaching a
// node restarts its CSS transition, flashing it visible. This guard proves that
// re-committing an unchanged keyed subtree performs ZERO DOM moves of that
// persisted node — so no transition/animation can replay from an unrelated
// state change. It is provider-agnostic: it does not depend on the desktop app,
// only on the render-dom commit contract.
describe("commit idempotency: unrelated re-render never re-parents persisted keyed content", () => {
  const transcriptFrame = (composerLabel: string) =>
    Stack({ key: "shell", direction: "column" }, [
      Transcript({ key: "shell-transcript", messages: [
        {
          key: "m1",
          role: "assistant",
          timestamp: "12:00",
          body: [
            Text({ key: "m1-text", content: "hello", variant: "body" }),
            Stack({ key: "note-meta-row-m1", direction: "row" }, [
              Button({
                key: "note-details-m1",
                label: "details",
                variant: "ghost",
                onPress: IntentRef("DesktopMessageSelected")
              })
            ])
          ]
        }
      ] }),
      // The sibling that changes on every keystroke (composer value binding).
      Text({ key: "shell-composer", content: composerLabel, variant: "body" })
    ])

  test("the hover-reveal details affordance is not moved when only a sibling changes", async () => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const root = document.createElement("div")
    document.body.appendChild(root)
    const report: IntentReporter = () => Effect.succeed(undefined)

    // Count MOVES of the persisted details button across commits. A move
    // (re-parent of an already-attached node) is exactly what restarts the CSS
    // opacity transition; the initial mount attach (parentNode === null) is not
    // a move. With the pre-fix renderer this was > 0 on the second commit.
    const isDetails = (n: unknown): n is Element =>
      typeof (n as Element)?.getAttribute === "function" &&
      (n as Element).getAttribute("data-en-key") === "note-details-m1"
    let detailsMoves = 0
    const proto = (window as unknown as { Node: { prototype: Record<string, (...a: Array<unknown>) => unknown> } }).Node.prototype
    for (const method of ["appendChild", "append", "insertBefore"] as const) {
      const real = proto[method]
      proto[method] = function (this: Node, ...args: Array<unknown>) {
        for (const arg of args) {
          if (isDetails(arg) && (arg as Node).parentNode != null && (arg as Node).parentNode !== this) {
            detailsMoves += 1
          }
        }
        return real.apply(this, args)
      }
    }

    await Effect.runPromise(Effect.scoped(Effect.gen(function*() {
      yield* makeDomRenderer({ document }).mount(
        root,
        // Two commits: identical transcript, only the composer sibling changes —
        // exactly the "type a character in the input" scenario.
        Stream.make(transcriptFrame("a"), transcriptFrame("ab")),
        report
      )
      yield* nextTask
      const button = root.querySelector('[data-en-key="note-details-m1"]')
      expect(button).not.toBeNull()
      // The affordance survived both commits without being re-parented, so its
      // hover-reveal transition can never replay from an unrelated re-render.
      expect(detailsMoves).toBe(0)
    })))
  })
})
