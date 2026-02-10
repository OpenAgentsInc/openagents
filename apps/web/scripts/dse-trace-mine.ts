import { parseTraceMineArgs, runTraceMine } from "./dse-trace-mine-lib"

const main = async () => {
  const parsed = parseTraceMineArgs(process.argv.slice(2))
  if (!parsed.ok) {
    console.error(parsed.error)
    console.error(parsed.usage)
    process.exitCode = 1
    return
  }

  const secret = process.env.OA_DSE_ADMIN_SECRET ?? ""
  if (!secret) {
    console.error("missing env: OA_DSE_ADMIN_SECRET")
    process.exitCode = 1
    return
  }

  const summary = await runTraceMine({ options: parsed.options, env: { OA_DSE_ADMIN_SECRET: secret }, fetchFn: fetch })
  console.log(JSON.stringify(summary, null, 2))
  process.exitCode = summary.ok ? 0 : 1
}

main().catch((err) => {
  console.error(String(err?.stack ?? err))
  process.exitCode = 1
})

