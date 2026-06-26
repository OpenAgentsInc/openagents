import type { CliRenderer, InputRenderable as InputRenderableType, TextRenderable as TextRenderableType } from "@opentui/core"

interface OpentuiCore {
  readonly createCliRenderer: (options: {
    readonly exitOnCtrlC: boolean
    readonly targetFps: number
    readonly screenMode: string
  }) => Promise<CliRenderer>
  readonly InputRenderable: typeof InputRenderableType
  readonly InputRenderableEvents: {
    readonly ENTER: string
  }
  readonly TextRenderable: typeof TextRenderableType
}

export async function readPromptWithOpenTui(): Promise<string | null> {
  const core = await importOpentuiCore()
  const renderer = await core.createCliRenderer({
    exitOnCtrlC: true,
    targetFps: 30,
    screenMode: "alternate-screen",
  })

  return await new Promise<string | null>((resolve) => {
    let settled = false
    const settle = (value: string | null) => {
      if (settled) return
      settled = true
      try {
        renderer.destroy()
      } finally {
        resolve(value)
      }
    }

    const title = new core.TextRenderable(renderer, {
      id: "khala-title",
      content: "Khala",
      position: "absolute",
      left: 2,
      top: 1,
      width: Math.max(20, renderer.width - 4),
      height: 1,
      fg: "#E6EDF3",
    })

    const input = new core.InputRenderable(renderer, {
      id: "khala-input",
      position: "absolute",
      left: 2,
      top: 3,
      width: Math.max(20, renderer.width - 4),
      zIndex: 10,
      backgroundColor: "#0D1117",
      textColor: "#E6EDF3",
      placeholder: "Ask Khala, or type /exit",
      placeholderColor: "#7D8590",
      cursorColor: "#58A6FF",
      value: "",
      maxLength: 8_000,
    })

    input.on(core.InputRenderableEvents.ENTER, (value: string) => {
      settle(value)
    })
    renderer.keyInput.on("keypress", (key: { readonly ctrl?: boolean; readonly name?: string }) => {
      if (key.ctrl && key.name === "c") {
        settle(null)
      }
    })

    renderer.root.add(title)
    renderer.root.add(input)
    input.focus()
    renderer.start()
  })
}

async function importOpentuiCore(): Promise<OpentuiCore> {
  const specifier = ["@opentui", "core"].join("/")
  return (await import(specifier)) as unknown as OpentuiCore
}
