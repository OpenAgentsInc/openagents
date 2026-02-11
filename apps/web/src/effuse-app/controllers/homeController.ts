import { Cause, Effect, Exit, Fiber } from "effect"

import { SessionAtom } from "../../effect/atoms/session"
import { formatCountdown } from "../../effuse-pages/home"
import {
  cleanupMarketingDotsGridBackground,
  hydrateMarketingDotsGridBackground,
} from "../../effuse-pages/marketingShell"

import { openChatPaneOnHome } from "./home/openChatPaneController"
import type { HomeChatDeps } from "./home/types"

import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"
import type { ChatClient } from "../../effect/chat"
import type { AppRuntime } from "../../effect/runtime"

export type HomeController = {
  readonly cleanup: () => void
}

const toStructuredError = (error: unknown): unknown => {
  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      stack: error.stack,
    }
  }
  return error
}

const logHomeControllerAsyncError = (context: string, error: unknown): void => {
  console.error("[homeController] async_failure", {
    context,
    error: toStructuredError(error),
  })
}

function startPrelaunchCountdownTicker(container: Element): () => void {
  const wrapper = container.querySelector("[data-prelaunch-countdown]")
  const display = container.querySelector("[data-countdown-display]")
  const targetAttr = wrapper?.getAttribute("data-countdown-target")
  if (!display || !targetAttr) return () => { }
  const targetMs = Number(targetAttr)
  if (Number.isNaN(targetMs)) return () => { }

  const tick = () => {
    const left = targetMs - Date.now()
    display.textContent = formatCountdown(Math.max(0, left))
  }
  tick()
  const id = setInterval(tick, 1000)
  return () => clearInterval(id)
}

export const mountHomeController = (input: {
  readonly container: Element
  readonly runtime?: AppRuntime
  readonly atoms?: AtomRegistry
  readonly navigate?: (href: string) => void
  readonly signOut?: () => void | Promise<void>
  readonly chat?: ChatClient
  readonly refreshConvexAuth?: () => void | Promise<void>
}): HomeController => {
  const hydrateFiber = Effect.runFork(hydrateMarketingDotsGridBackground(input.container))
  void Effect.runPromise(Fiber.await(hydrateFiber)).then(
    (exit) => {
      if (Exit.isFailure(exit) && !Cause.isInterruptedOnly(exit.cause)) {
        logHomeControllerAsyncError("home.marketing_background.hydrate", Cause.pretty(exit.cause))
      }
    },
    (error) => {
      logHomeControllerAsyncError("home.marketing_background.hydrate", error)
    },
  )

  const stopCountdown = startPrelaunchCountdownTicker(input.container)

  let deps: HomeChatDeps | undefined
  if (input.runtime && input.atoms && input.navigate && input.signOut && input.chat) {
    const atoms = input.atoms
    deps = {
      runtime: input.runtime,
      atoms,
      sessionState: {
        read: () => atoms.get(SessionAtom),
        write: (session) => atoms.set(SessionAtom, session),
      },
      navigate: input.navigate,
      signOut: input.signOut,
      chat: input.chat,
      refreshConvexAuth: input.refreshConvexAuth,
    }
  }

  const stopOpenChatPane = openChatPaneOnHome(input.container, deps)

  return {
    cleanup: () => {
      Effect.runFork(Fiber.interrupt(hydrateFiber))
      stopCountdown()
      stopOpenChatPane()
      cleanupMarketingDotsGridBackground(input.container)
    },
  }
}
