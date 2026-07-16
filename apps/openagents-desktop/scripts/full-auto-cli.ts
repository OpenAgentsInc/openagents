/**
 * FA-H13 (#8886): a tiny argv CLI over the Full Auto local control API -- a
 * deliberately thin pass-through client of the OpenAPI surface Desktop main
 * serves. Prints the server's JSON verbatim; exits nonzero on any non-2xx.
 *
 * Usage (package script `pnpm --dir apps/openagents-desktop run full-auto`):
 *   node --import tsx scripts/full-auto-cli.ts list
 *   node --import tsx scripts/full-auto-cli.ts status <threadRef>
 *   node --import tsx scripts/full-auto-cli.ts enable <threadRef> --workspace <path>
 *   node --import tsx scripts/full-auto-cli.ts disable <threadRef>
 *   node --import tsx scripts/full-auto-cli.ts continue-now <threadRef>
 *   node --import tsx scripts/full-auto-cli.ts turns <threadRef>
 *   node --import tsx scripts/full-auto-cli.ts openapi
 * Options: --user-data <path> (or OPENAGENTS_DESKTOP_USER_DATA) when Desktop
 * runs against a non-default userData directory.
 */
import {
  ControlUnavailableError,
  controlOperations,
  readControlConnection,
  resolveUserDataDir,
} from "./full-auto-control-client.ts"

const USAGE = `usage: full-auto-cli <command> [args] [--user-data <path>]
commands:
  list
  status <threadRef>
  enable <threadRef> --workspace <path>
  disable <threadRef>
  continue-now <threadRef>
  turns <threadRef>
  openapi`

const main = async (): Promise<void> => {
  const argv = [...process.argv.slice(2)]
  const takeOption = (name: string): string | undefined => {
    const index = argv.indexOf(name)
    if (index === -1) return undefined
    const value = argv[index + 1]
    argv.splice(index, 2)
    return value
  }
  const userData = takeOption("--user-data")
  const workspace = takeOption("--workspace")
  const [command, threadRef] = argv

  const connection = readControlConnection(resolveUserDataDir(userData))
  const operations = controlOperations(connection)

  const requireThreadRef = (): string => {
    if (threadRef === undefined || threadRef.length === 0) {
      console.error(`${command}: a <threadRef> argument is required\n${USAGE}`)
      process.exit(2)
    }
    return threadRef
  }

  const result = command === "list"
    ? await operations.list()
    : command === "openapi"
    ? await operations.openapi()
    : command === "status"
    ? await operations.status(requireThreadRef())
    : command === "enable"
    ? await (async () => {
        const ref = requireThreadRef()
        if (workspace === undefined || workspace.length === 0) {
          console.error(`enable: --workspace <path> is required (enable must NAME the workspace it expects)\n${USAGE}`)
          process.exit(2)
        }
        return operations.enable(ref, workspace)
      })()
    : command === "disable"
    ? await operations.disable(requireThreadRef())
    : command === "continue-now"
    ? await operations.continueNow(requireThreadRef())
    : command === "turns"
    ? await operations.turns(requireThreadRef())
    : null

  if (result === null) {
    console.error(USAGE)
    process.exit(2)
  }
  console.log(JSON.stringify(result.body, null, 2))
  if (result.status < 200 || result.status >= 300) process.exit(1)
}

await main().catch(error => {
  if (error instanceof ControlUnavailableError) {
    console.error(error.message)
  } else {
    console.error(
      "full-auto-cli failed:",
      error instanceof Error ? error.message : String(error),
    )
  }
  process.exit(1)
})
