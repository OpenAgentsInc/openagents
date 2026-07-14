import { runCheck } from "./check-workspace"

const mode = process.argv[2] ?? "check"

try {
  await runCheck(process.cwd(), mode)
  console.error(`\n[check] ${mode} green`)
} catch (error) {
  console.error(`\n[check] ${error instanceof Error ? error.message : String(error)}`)
  process.exitCode = 1
}
