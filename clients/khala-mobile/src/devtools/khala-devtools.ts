export const khalaDevtoolCommandNames = [
  "inspectConnectivity",
  "jumpToFixtureThread",
  "resetFixtureState",
  "resetNavigation",
  "seedFixtureThreads",
] as const

export type KhalaDevtoolCommandName = (typeof khalaDevtoolCommandNames)[number]

export type KhalaDevtoolsCommandResult = Readonly<{
  ok: boolean
  messageSafe: string
}>

export type KhalaDevtoolsNavigationAdapter = Readonly<{
  jumpToThread: (thread: { readonly threadId: string; readonly title: string }) => void
  resetToThreads: () => void
}>

export type KhalaDevtoolsFixtureAdapter = Readonly<{
  resetFixtureState: () => void | Promise<void>
  seedFixtureThreads: () => void | Promise<void>
}>

export type KhalaDevtoolsConnectivitySnapshot = Readonly<{
  reachable: boolean
  targetKind: "simulator_loopback" | "tailnet" | "unknown"
}>

export type KhalaDevtoolsConnectivityAdapter = () =>
  | KhalaDevtoolsConnectivitySnapshot
  | Promise<KhalaDevtoolsConnectivitySnapshot>

export type KhalaMobileDevtools = Readonly<{
  available: boolean
  commands: ReadonlyArray<KhalaDevtoolCommandName>
  execute: (command: KhalaDevtoolCommandName) => Promise<KhalaDevtoolsCommandResult>
}>

const fixtureThread = {
  threadId: "thread.fixture.public",
  title: "Fixture thread",
} as const

export const isKhalaMobileDevtoolsEnabled = (): boolean =>
  typeof __DEV__ === "boolean" && __DEV__ === true

const unavailable = async (): Promise<KhalaDevtoolsCommandResult> => ({
  messageSafe: "Khala mobile devtools are unavailable in production builds.",
  ok: false,
})

export const createKhalaMobileDevtools = (input: {
  readonly connectivity?: KhalaDevtoolsConnectivityAdapter
  readonly dev: boolean
  readonly fixtures?: KhalaDevtoolsFixtureAdapter
  readonly navigation?: KhalaDevtoolsNavigationAdapter
}): KhalaMobileDevtools => {
  if (!input.dev) {
    return {
      available: false,
      commands: [],
      execute: unavailable,
    }
  }

  return {
    available: true,
    commands: khalaDevtoolCommandNames,
    execute: async command => {
      switch (command) {
        case "inspectConnectivity": {
          const snapshot = await input.connectivity?.()
          return {
            messageSafe:
              snapshot === undefined
                ? "Connectivity snapshot unavailable."
                : `Connectivity ${snapshot.reachable ? "reachable" : "unreachable"} via ${snapshot.targetKind}.`,
            ok: snapshot !== undefined,
          }
        }
        case "jumpToFixtureThread":
          input.navigation?.jumpToThread(fixtureThread)
          return { messageSafe: "Opened public fixture thread.", ok: input.navigation !== undefined }
        case "resetFixtureState":
          await input.fixtures?.resetFixtureState()
          return { messageSafe: "Reset public fixture state.", ok: input.fixtures !== undefined }
        case "resetNavigation":
          input.navigation?.resetToThreads()
          return { messageSafe: "Reset navigation to Threads.", ok: input.navigation !== undefined }
        case "seedFixtureThreads":
          await input.fixtures?.seedFixtureThreads()
          return { messageSafe: "Seeded public fixture threads.", ok: input.fixtures !== undefined }
      }
    },
  }
}

export const khalaMobileDevtools = createKhalaMobileDevtools({
  dev: isKhalaMobileDevtoolsEnabled(),
})
