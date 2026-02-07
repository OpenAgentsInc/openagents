import { Effect } from "effect"

import {
  cleanupMarketingDotsGridBackground,
  hydrateMarketingDotsGridBackground,
} from "../../effuse-pages/marketingShell"

export type HomeController = {
  readonly cleanup: () => void
}

export const mountHomeController = (input: {
  readonly container: Element
}): HomeController => {
  Effect.runPromise(hydrateMarketingDotsGridBackground(input.container)).catch(() => {})

  return {
    cleanup: () => {
      cleanupMarketingDotsGridBackground(input.container)
    },
  }
}

