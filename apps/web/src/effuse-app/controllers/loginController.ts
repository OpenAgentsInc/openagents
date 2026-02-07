import { Effect } from "effect"
import type { EzAction } from "@openagentsinc/effuse"

import { AuthService, clearAuthClientCache } from "../../effect/auth"
import { SessionAtom } from "../../effect/atoms/session"
import { runLoginPage } from "../../effuse-pages/login"

import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry"
import type { AppRuntime } from "../../effect/runtime"
import type { TelemetryClient } from "../../effect/telemetry"
import type { LoginPageModel, LoginStep } from "../../effuse-pages/login"

const normalizeEmail = (raw: string): string => raw.trim().toLowerCase()
const normalizeCode = (raw: string): string => raw.replace(/\s+/g, "")

export type LoginController = {
  readonly cleanup: () => void
}

export const mountLoginController = (input: {
  readonly container: Element
  readonly ez: Map<string, EzAction>
  readonly runtime: AppRuntime
  readonly atoms: AtomRegistry
  readonly telemetry: TelemetryClient
  readonly navigate: (href: string) => void
}): LoginController => {
  let step: LoginStep = "email"
  let email = ""
  let code = ""
  let isBusy = false
  let errorText: string | null = null

  let lastRenderKey = `${step}:${isBusy ? 1 : 0}:${errorText ?? ""}`

  const model = (): LoginPageModel => ({
    step,
    email,
    code,
    isBusy,
    errorText,
  })

  const renderIfNeeded = () => {
    const nextKey = `${step}:${isBusy ? 1 : 0}:${errorText ?? ""}`
    if (nextKey === lastRenderKey) return
    lastRenderKey = nextKey
    Effect.runPromise(runLoginPage(input.container, model())).catch(() => {})
  }

  const setBusy = (busy: boolean) => {
    isBusy = busy
    renderIfNeeded()
  }

  const setError = (text: string | null) => {
    errorText = text
    renderIfNeeded()
  }

  const setStep = (next: LoginStep) => {
    step = next
    renderIfNeeded()
  }

  input.ez.set("login.email.input", ({ params }) =>
    Effect.sync(() => {
      email = String((params as any).email ?? "")
    })
  )

  input.ez.set("login.code.input", ({ params }) =>
    Effect.sync(() => {
      code = String((params as any).code ?? "")
    })
  )

  input.ez.set("login.email.submit", ({ params }) =>
    Effect.gen(function* () {
      if (isBusy) return
      const nextEmail = normalizeEmail(String((params as any).email ?? email))
      if (!nextEmail) return

      yield* Effect.sync(() => {
        setError(null)
        setBusy(true)
      })

      const exit = yield* Effect.tryPromise({
        try: async () => {
          const r = await fetch("/api/auth/start", {
            method: "POST",
            headers: { "content-type": "application/json" },
            credentials: "include",
            body: JSON.stringify({ email: nextEmail }),
          })
          const data: any = await r.json().catch(() => null)
          if (!r.ok || !data?.ok) {
            throw new Error(typeof data?.error === "string" ? data.error : "send_failed")
          }
        },
        catch: (err) => (err instanceof Error ? err : new Error(String(err))),
      }).pipe(Effect.exit)

      if (exit._tag === "Failure") {
        const message = String(exit.cause)
        Effect.runPromise(
          input.telemetry.withNamespace("ui.login").event("login.start_failed", { message })
        ).catch(() => {})

        yield* Effect.sync(() => {
          setError(message === "invalid_email" ? "Please enter a valid email." : "Failed to send code. Try again.")
          setBusy(false)
        })
        return
      }

      yield* Effect.sync(() => {
        email = nextEmail
        code = ""
        setStep("code")
        setBusy(false)
      })
    })
  )

  input.ez.set("login.code.submit", ({ params }) =>
    Effect.gen(function* () {
      if (isBusy) return
      const nextEmail = normalizeEmail(email)
      if (!nextEmail) return
      const nextCode = normalizeCode(String((params as any).code ?? code))
      if (!nextCode) return

      yield* Effect.sync(() => {
        setError(null)
        setBusy(true)
      })

      const exit = yield* Effect.tryPromise({
        try: async () => {
          const r = await fetch("/api/auth/verify", {
            method: "POST",
            headers: { "content-type": "application/json" },
            credentials: "include",
            body: JSON.stringify({ email: nextEmail, code: nextCode }),
          })
          const data: any = await r.json().catch(() => null)
          if (!r.ok || !data?.ok) {
            throw new Error(typeof data?.error === "string" ? data.error : "verify_failed")
          }
        },
        catch: (err) => (err instanceof Error ? err : new Error(String(err))),
      }).pipe(Effect.exit)

      if (exit._tag === "Failure") {
        const message = String(exit.cause)
        Effect.runPromise(
          input.telemetry.withNamespace("ui.login").event("login.verify_failed", { message })
        ).catch(() => {})

        yield* Effect.sync(() => {
          setError(message === "invalid_code" ? "Invalid code. Please try again." : "Verification failed. Try again.")
          setBusy(false)
        })
        return
      }

      clearAuthClientCache()

      const sessionExit = yield* Effect.promise(() =>
        input.runtime.runPromiseExit(Effect.flatMap(AuthService, (auth) => auth.getSession()))
      )

      if (sessionExit._tag === "Success") {
        const session = sessionExit.value
        input.atoms.set(SessionAtom as any, {
          userId: session.userId,
          user: session.user
            ? {
                id: session.user.id,
                email: session.user.email,
                firstName: session.user.firstName,
                lastName: session.user.lastName,
              }
            : null,
        })

        if (session.userId) {
          Effect.runPromise(
            input.telemetry
              .withNamespace("auth.workos")
              .identify(session.userId, { userId: session.userId })
          ).catch(() => {})
        }
      }

      yield* Effect.sync(() => setBusy(false))
      yield* Effect.sync(() => input.navigate("/autopilot"))
    })
  )

  input.ez.set("login.code.back", () =>
    Effect.sync(() => {
      if (isBusy) return
      setError(null)
      code = ""
      setStep("email")
    })
  )

  input.ez.set("login.code.resend", () =>
    Effect.gen(function* () {
      if (isBusy) return
      const nextEmail = normalizeEmail(email)
      if (!nextEmail) return

      yield* Effect.sync(() => {
        setError(null)
        setBusy(true)
      })

      const exit = yield* Effect.tryPromise({
        try: async () => {
          const r = await fetch("/api/auth/start", {
            method: "POST",
            headers: { "content-type": "application/json" },
            credentials: "include",
            body: JSON.stringify({ email: nextEmail }),
          })
          const data: any = await r.json().catch(() => null)
          if (!r.ok || !data?.ok) {
            throw new Error(typeof data?.error === "string" ? data.error : "send_failed")
          }
        },
        catch: (err) => (err instanceof Error ? err : new Error(String(err))),
      }).pipe(Effect.exit)

      if (exit._tag === "Failure") {
        const message = String(exit.cause)
        Effect.runPromise(
          input.telemetry.withNamespace("ui.login").event("login.resend_failed", { message })
        ).catch(() => {})
        yield* Effect.sync(() => setError("Failed to resend code. Try again."))
      }

      yield* Effect.sync(() => setBusy(false))
    })
  )

  return {
    cleanup: () => {
      input.ez.delete("login.email.input")
      input.ez.delete("login.code.input")
      input.ez.delete("login.email.submit")
      input.ez.delete("login.code.submit")
      input.ez.delete("login.code.back")
      input.ez.delete("login.code.resend")
    },
  }
}

