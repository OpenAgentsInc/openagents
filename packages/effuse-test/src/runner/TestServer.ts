import * as ChildProcess from "node:child_process"

import { Effect, Scope } from "effect"

const spawnLogged = (cwd: string, cmd: string, args: ReadonlyArray<string>) =>
  Effect.async<ChildProcess.ChildProcess, Error>((resume) => {
    const child = ChildProcess.spawn(cmd, [...args], {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
      env: process.env,
      detached: true,
    })

    child.once("error", (err) => resume(Effect.fail(err)))
    // Resolve immediately; readiness is checked separately.
    resume(Effect.succeed(child))
  })

const waitForHttpOk = (url: string, timeoutMs: number) =>
  Effect.gen(function* () {
    const start = Date.now()
    // eslint-disable-next-line no-constant-condition
    while (true) {
      const ok = yield* Effect.tryPromise({
        try: () => fetch(url, { redirect: "manual" }),
        catch: (e) => (e instanceof Error ? e : new Error(String(e))),
      }).pipe(
        Effect.map((res) => res.ok),
        Effect.catchAll(() => Effect.succeed(false)),
      )
      if (ok) return
      if (Date.now() - start > timeoutMs) {
        return yield* Effect.fail(new Error(`Timed out waiting for ${url}`))
      }
      yield* Effect.sleep("100 millis")
    }
  })

export type TestServer = {
  readonly baseUrl: string
}

type TestServerInternal = TestServer & { readonly _child: ChildProcess.ChildProcess }

export const startWranglerDev = (options: {
  readonly projectDir: string
  readonly port: number
}): Effect.Effect<TestServer, Error, Scope.Scope> =>
  Effect.acquireRelease(
    Effect.gen(function* () {
      const baseUrl = `http://127.0.0.1:${options.port}`

      // Build assets first so the Worker can serve /effuse-client.* from ASSETS.
      yield* Effect.logInfo(`Building effuse client in ${options.projectDir}`)
      yield* spawnLogged(options.projectDir, "npm", ["run", "build:effuse-client"]).pipe(
        Effect.flatMap((child) =>
          Effect.async<void, Error>((resume) => {
            child.stdout?.on("data", (d) => process.stdout.write(d))
            child.stderr?.on("data", (d) => process.stderr.write(d))
            child.once("exit", (code) => {
              if (code === 0) resume(Effect.void)
              else resume(Effect.fail(new Error(`npm build failed (code=${code})`)))
            })
          }),
        ),
      )

      yield* Effect.logInfo(`Starting wrangler dev on ${baseUrl}`)
      const child = yield* spawnLogged(options.projectDir, "npx", [
        "wrangler",
        "dev",
        "--port",
        String(options.port),
        "--local",
        "--ip",
        "127.0.0.1",
      ])

      child.stdout?.on("data", (d) => process.stdout.write(d))
      child.stderr?.on("data", (d) => process.stderr.write(d))

      yield* waitForHttpOk(`${baseUrl}/`, 60_000)
      const internal: TestServerInternal = { baseUrl, _child: child }
      return internal
    }),
    ({ _child }) =>
      Effect.sync(() => {
        try {
          if (_child.pid != null) process.kill(-_child.pid, "SIGTERM")
          else _child.kill("SIGTERM")
        } catch {
          // ignore
        }
      }),
  ).pipe(Effect.map(({ baseUrl }) => ({ baseUrl })))
