import { Effect } from "effect"

import { cleanupAuthedDotsGridBackground, hydrateAuthedDotsGridBackground, runAuthedShell } from "../../effuse-pages/authedShell"
import { modulesPageTemplate } from "../../effuse-pages/modules"
import { signaturesPageTemplate } from "../../effuse-pages/signatures"
import { toolsPageTemplate } from "../../effuse-pages/tools"
import { ModulesPageDataAtom, SignaturesPageDataAtom, ToolsPageDataAtom } from "../../effect/atoms/contracts"
import { SessionAtom } from "../../effect/atoms/session"

import type { Atom } from "@effect-atom/atom"
import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"
import type { ModulesPageData } from "../../effuse-pages/modules"
import type { SignaturesPageData } from "../../effuse-pages/signatures"
import type { ToolsPageData } from "../../effuse-pages/tools"

export type ContractsController = {
  readonly cleanup: () => void
}

const mountAuthedContractsController = <A>(input: {
  readonly container: Element
  readonly atoms: AtomRegistry
  readonly pageAtomForUser: (userId: string) => Atom.Atom<A>
  readonly render: (data: A) => Effect.Effect<void>
}): ContractsController => {
  let unsubPage: (() => void) | null = null

  const stopPage = () => {
    if (unsubPage) {
      unsubPage()
      unsubPage = null
    }
  }

  const startForUser = (userId: string) => {
    stopPage()
    unsubPage = input.atoms.subscribe(
      input.pageAtomForUser(userId),
      (data) => {
        Effect.runPromise(input.render(data)).catch(() => {})
      },
      { immediate: true },
    )
  }

  const unsubSession = input.atoms.subscribe(
    SessionAtom,
    (session) => {
      if (session.userId) startForUser(session.userId)
    },
    { immediate: true },
  )

  Effect.runPromise(hydrateAuthedDotsGridBackground(input.container)).catch(() => {})

  return {
    cleanup: () => {
      unsubSession()
      stopPage()
      cleanupAuthedDotsGridBackground(input.container)
    },
  }
}

export const mountModulesController = (input: {
  readonly container: Element
  readonly atoms: AtomRegistry
}): ContractsController =>
  mountAuthedContractsController<ModulesPageData>({
    container: input.container,
    atoms: input.atoms,
    pageAtomForUser: (userId) => ModulesPageDataAtom(userId),
    render: (data) => runAuthedShell(input.container, modulesPageTemplate(data)),
  })

export const mountToolsController = (input: {
  readonly container: Element
  readonly atoms: AtomRegistry
}): ContractsController =>
  mountAuthedContractsController<ToolsPageData>({
    container: input.container,
    atoms: input.atoms,
    pageAtomForUser: (userId) => ToolsPageDataAtom(userId),
    render: (data) => runAuthedShell(input.container, toolsPageTemplate(data)),
  })

export const mountSignaturesController = (input: {
  readonly container: Element
  readonly atoms: AtomRegistry
}): ContractsController =>
  mountAuthedContractsController<SignaturesPageData>({
    container: input.container,
    atoms: input.atoms,
    pageAtomForUser: (userId) => SignaturesPageDataAtom(userId),
    render: (data) => runAuthedShell(input.container, signaturesPageTemplate(data)),
  })

