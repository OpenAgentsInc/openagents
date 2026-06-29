import { Electroview } from "electrobun/view"

import {
  OPENAGENTS_DESKTOP_PYLON_POLL_INTERVAL_MS,
  type PylonStatusResult,
} from "../shared/pylon-status"
import {
  OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type OpenAgentsDesktopRPCSchema,
} from "../shared/rpc"
import { mountLandingSquares } from "./landing-squares"
import "./styles.css"

const rpc = Electroview.defineRPC<OpenAgentsDesktopRPCSchema>({
  maxRequestTime: OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {},
    messages: {},
  },
})

new Electroview({ rpc })

const scene = document.querySelector<HTMLElement>("#openagents-scene")
if (scene === null) {
  throw new Error("Missing #openagents-scene mount")
}

const pylonStatus = document.querySelector<HTMLElement>("#pylon-status")
if (pylonStatus === null) {
  throw new Error("Missing #pylon-status")
}

const pylonCount = document.querySelector<HTMLElement>("#pylon-count")
if (pylonCount === null) {
  throw new Error("Missing #pylon-count")
}

const prefersReducedMotion = globalThis.matchMedia?.(
  "(prefers-reduced-motion: reduce)",
).matches ?? false

const handle = mountLandingSquares(scene, {
  animate: !prefersReducedMotion,
  pose: "landing",
})

globalThis.addEventListener("pagehide", () => {
  handle.dispose()
})

const formatCount = (value: number): string =>
  new Intl.NumberFormat("en-US").format(Math.max(0, Math.trunc(value)))

const renderPylonStatus = (result: PylonStatusResult): void => {
  pylonCount.textContent = `Pylons: ${formatCount(result.count)}`
  pylonStatus.dataset.state =
    result.ok && result.count > 0 ? "online" : result.ok ? "empty" : "unknown"
  pylonStatus.title = result.ok ? "Pylon status" : result.error
}

const loadPylonStatus = async (): Promise<void> => {
  try {
    renderPylonStatus(await rpc.request.pylonStatus())
  } catch (error) {
    renderPylonStatus({
      ok: false,
      count: 0,
      error: error instanceof Error ? error.message : String(error),
      observedAt: new Date().toISOString(),
    })
  }
}

void loadPylonStatus()
globalThis.setInterval(
  () => void loadPylonStatus(),
  OPENAGENTS_DESKTOP_PYLON_POLL_INTERVAL_MS,
)
