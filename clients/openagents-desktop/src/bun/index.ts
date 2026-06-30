import { BrowserWindow } from "electrobun/bun"
import { Effect } from "effect"

const openBlankWindow = Effect.sync(
  () =>
    new BrowserWindow({
      title: "OpenAgents",
      url: "views://openagents-desktop/index.html",
      frame: { x: 128, y: 96, width: 1024, height: 720 },
    }),
)

if (Bun.env.OPENAGENTS_DESKTOP_OPEN_WINDOW !== "0") {
  Effect.runSync(openBlankWindow)
}
