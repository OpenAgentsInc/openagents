import { describe, expect, test } from "bun:test"
import { Window } from "../../../apps/web/node_modules/happy-dom"
import {
  Button,
  CodeEditor,
  decodeCodeEditorHostProps,
  Icon,
  IntentRef,
  List,
  SplitPane,
  Stack,
  Text,
  UnknownIntentError,
  iconNames,
  type IntentReporter
} from "@effect-native/core"
import { Effect, Stream } from "@effect-native/core/effect"
import { Deferred } from "effect"

import { makeDomRenderer, makeStubCodeEditorDriver, type DomHostContext, type DomHostDriver } from "../src/index"

const nextTask = Effect.promise<void>(
  () => new Promise((resolve) => setTimeout(resolve, 0))
)

describe("DOM renderer host boundaries", () => {
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
