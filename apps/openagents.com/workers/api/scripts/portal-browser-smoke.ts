import { Runtime } from "@openagentsinc/runtime-platform"
// PORTAL-1 (#8652 reopen): REAL-BROWSER smoke for openagents.com/portal.
//
// Why this exists: the original PORTAL-1 closeout proved the client path with
// curl + DOM-unit tests only. The owner then loaded /portal in a real browser,
// logged in, and hit an authenticated empty state with no account context —
// shipped broken. This script is the binding browser-level gate: it drives the
// DEPLOYED surface with headless Chromium and captures screenshot receipts of
// the owner-visible states. Curl-level checks are never again sufficient
// evidence that /portal "works".
//
// States proven (each produces a PNG receipt in --out-dir):
//   1. logged-out            -> login gate visible ("Log in with GitHub"),
//                               never the empty/engagement body.
//                               Runs with NO credentials; wired into
//                               deploy-cloudrun.sh on every deploy.
//   2. logged-in-empty       -> "Your setup is being prepared" + the
//                               signed-in account email ("Signed in as ...")
//                               + the "Sign out / switch account" affordance.
//   3. logged-in-engagement  -> engagement header + "Content calendar"
//                               (the real demo content).
//
// Logged-in states need a browser session. Provide ONE of:
//   --cookie "oa_access=...; oa_refresh=..."   (or env PORTAL_SMOKE_COOKIE;
//       grab the pair from devtools of a logged-in browser — never commit it)
//   --login-email you@example.com --otp-command "shell cmd printing the code"
//       (drives the real /login/email OpenAuth flow; the otp-command is
//       polled until it prints the one-time code, e.g. a Gmail CLI read)
//
// Usage:
//   node --import tsx scripts/portal-browser-smoke.ts --base-url https://openagents.com \
//     --out-dir /tmp/portal-smoke --state logged-out
//   PORTAL_SMOKE_COOKIE='oa_access=...; oa_refresh=...' \
//     node --import tsx scripts/portal-browser-smoke.ts --state logged-in --expect empty
//   node --import tsx scripts/portal-browser-smoke.ts --state logged-in --expect engagement \
//     --login-email chris@openagents.com \
//     --otp-command 'bash scripts/read-signin-code.sh'
//
// Requires playwright's chromium once per machine:
//   pnpm exec playwright install chromium

import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import { chromium, type BrowserContext, type Page } from 'playwright'

const LOGIN_GATE_TEXT = 'Log in to view your engagement.'
const LOGIN_BUTTON_TEXT = 'Log in with GitHub'
const EMPTY_TITLE_TEXT = 'Your setup is being prepared'
const EMPTY_IDENTITY_TEXT = 'Signed in as'
const EMPTY_SWITCH_TEXT = 'Sign out / switch account'
// NOTE: not "Content calendar" — the login gate copy mentions "content
// calendar" and playwright's string matching is case-insensitive.
const ENGAGEMENT_TEXT = 'Funnel KPIs'

type SmokeState = 'logged-out' | 'logged-in'
type LoggedInExpectation = 'empty' | 'engagement' | 'auto'

const parseArgs = (argv: ReadonlyArray<string>) => {
  const args: Record<string, string> = {}
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index]
    if (key?.startsWith('--')) {
      const value = argv[index + 1]
      if (value !== undefined && !value.startsWith('--')) {
        args[key.slice(2)] = value
        index += 1
      } else {
        args[key.slice(2)] = 'true'
      }
    }
  }
  return args
}

const fail = (message: string): never => {
  console.error(`PORTAL BROWSER SMOKE FAILED: ${message}`)
  process.exit(1)
}

const textVisible = async (page: Page, text: string, timeoutMs = 30_000) => {
  await page
    .getByText(text, { exact: false })
    .first()
    .waitFor({ state: 'visible', timeout: timeoutMs })
}

const textAbsent = async (page: Page, text: string) => {
  const count = await page.getByText(text, { exact: false }).count()
  if (count > 0) {
    fail(`expected "${text}" to be ABSENT but it rendered`)
  }
}

const addCookieHeader = async (
  context: BrowserContext,
  baseUrl: string,
  cookieHeader: string,
) => {
  const { hostname } = new URL(baseUrl)
  const cookies = cookieHeader
    .split(';')
    .map(pair => pair.trim())
    .filter(pair => pair.includes('='))
    .map(pair => {
      const eq = pair.indexOf('=')
      return {
        domain: hostname,
        httpOnly: true,
        name: pair.slice(0, eq).trim(),
        path: '/',
        sameSite: 'Lax' as const,
        secure: baseUrl.startsWith('https:'),
        value: pair.slice(eq + 1).trim(),
      }
    })
  if (cookies.length === 0) {
    fail('cookie header parsed to zero cookies')
  }
  await context.addCookies(cookies)
}

/** Drive the real /login/email OpenAuth code flow. The otp command is polled
 * (fresh shell each attempt) until it prints a 6+ digit code. */
const loginViaEmailOtp = async (
  page: Page,
  baseUrl: string,
  email: string,
  otpCommand: string,
) => {
  await page.goto(`${baseUrl}/login/email?returnTo=%2Fportal`, {
    waitUntil: 'domcontentloaded',
  })
  const emailInput = page.locator('input[name="email"], input[type="email"]').first()
  await emailInput.waitFor({ state: 'visible', timeout: 30_000 })
  const requestedAt = Date.now()
  await emailInput.fill(email)
  // OpenAuth's hosted UI button carries no type attribute; match in-form.
  await page.locator('form button, button[type="submit"]').first().click()

  const codeInput = page
    .locator('input[name="code"], input[autocomplete="one-time-code"], input[inputmode="numeric"]')
    .first()
  await codeInput.waitFor({ state: 'visible', timeout: 30_000 })

  let code: string | undefined
  const deadline = Date.now() + 120_000
  while (code === undefined && Date.now() < deadline) {
    const probe = Runtime.spawnSync(['bash', '-lc', otpCommand], {
      env: { ...process.env, PORTAL_SMOKE_OTP_REQUESTED_AT: String(requestedAt) },
    })
    const output = new TextDecoder().decode(probe.stdout).trim()
    const match = output.match(/\b(\d{6,8})\b/)
    if (match !== null) {
      code = match[1]
      break
    }
    await new Promise(resolve => setTimeout(resolve, 5_000))
  }
  if (code === undefined) {
    fail('otp-command never printed a sign-in code within 120s')
  }
  await codeInput.fill(code!)
  // OpenAuth's hosted UI button carries no type attribute; match in-form.
  await page.locator('form button, button[type="submit"]').first().click()
  await page.waitForURL(url => !url.pathname.startsWith('/auth'), {
    timeout: 60_000,
  })
}

const main = async () => {
  const args = parseArgs(process.argv.slice(2))
  const baseUrl = (args['base-url'] ?? 'https://openagents.com').replace(/\/+$/u, '')
  const outDir = args['out-dir'] ?? 'portal-smoke-receipts'
  const state: SmokeState = args['state'] === 'logged-in' ? 'logged-in' : 'logged-out'
  const expectation: LoggedInExpectation =
    args['expect'] === 'empty' || args['expect'] === 'engagement'
      ? args['expect']
      : 'auto'
  const cookieHeader = args['cookie'] ?? process.env.PORTAL_SMOKE_COOKIE
  const loginEmail = args['login-email']
  const otpCommand = args['otp-command']

  mkdirSync(outDir, { recursive: true })

  const browser = await chromium.launch()
  const results: Array<Record<string, unknown>> = []
  try {
    const context = await browser.newContext({
      viewport: { width: 1440, height: 1400 },
    })

    if (state === 'logged-out') {
      const page = await context.newPage()
      await page.goto(`${baseUrl}/portal`, { waitUntil: 'domcontentloaded' })
      await textVisible(page, LOGIN_GATE_TEXT)
      await textVisible(page, LOGIN_BUTTON_TEXT)
      await textAbsent(page, EMPTY_TITLE_TEXT)
      await textAbsent(page, ENGAGEMENT_TEXT)
      const shot = join(outDir, 'portal-logged-out.png')
      await page.screenshot({ path: shot, fullPage: true })
      results.push({ screenshot: shot, state: 'logged-out', ok: true })
    } else {
      if (cookieHeader !== undefined && cookieHeader !== '') {
        await addCookieHeader(context, baseUrl, cookieHeader)
      } else if (loginEmail !== undefined && otpCommand !== undefined) {
        const loginPage = await context.newPage()
        await loginViaEmailOtp(loginPage, baseUrl, loginEmail, otpCommand)
        await loginPage.close()
      } else {
        fail(
          'logged-in state needs --cookie/PORTAL_SMOKE_COOKIE or --login-email + --otp-command',
        )
      }

      const page = await context.newPage()
      await page.goto(`${baseUrl}/portal`, { waitUntil: 'domcontentloaded' })
      // Both authenticated outcomes; assert per expectation.
      if (expectation === 'empty') {
        await textVisible(page, EMPTY_TITLE_TEXT)
        await textVisible(page, EMPTY_IDENTITY_TEXT)
        await textVisible(page, EMPTY_SWITCH_TEXT)
        await textAbsent(page, ENGAGEMENT_TEXT)
        const shot = join(outDir, 'portal-logged-in-empty.png')
        await page.screenshot({ path: shot, fullPage: true })
        results.push({ screenshot: shot, state: 'logged-in-empty', ok: true })
      } else if (expectation === 'engagement') {
        await textVisible(page, ENGAGEMENT_TEXT)
        await textAbsent(page, EMPTY_TITLE_TEXT)
        await textAbsent(page, LOGIN_GATE_TEXT)
        const shot = join(outDir, 'portal-logged-in-engagement.png')
        await page.screenshot({ path: shot, fullPage: true })
        results.push({ screenshot: shot, state: 'logged-in-engagement', ok: true })
      } else {
        // auto: require one of the two honest authenticated states — and if
        // it is the empty state, the account identity MUST be visible.
        await Promise.race([
          textVisible(page, ENGAGEMENT_TEXT),
          textVisible(page, EMPTY_TITLE_TEXT),
        ])
        const isEmpty =
          (await page.getByText(EMPTY_TITLE_TEXT, { exact: false }).count()) > 0
        if (isEmpty) {
          await textVisible(page, EMPTY_IDENTITY_TEXT)
          await textVisible(page, EMPTY_SWITCH_TEXT)
        }
        const shot = join(
          outDir,
          isEmpty ? 'portal-logged-in-empty.png' : 'portal-logged-in-engagement.png',
        )
        await page.screenshot({ path: shot, fullPage: true })
        results.push({
          screenshot: shot,
          state: isEmpty ? 'logged-in-empty' : 'logged-in-engagement',
          ok: true,
        })
      }
    }
  } finally {
    await browser.close()
  }

  console.log(JSON.stringify({ baseUrl, ok: true, results }, null, 2))
}

main().catch(error => {
  fail(error instanceof Error ? (error.stack ?? error.message) : String(error))
})
