// Firm reason: 1 (it drives headless Chrome over DevTools: the page's cookies and its side's Log out are the browser's)
// The dev server's one account, checked headless (#13493): the side's name is the account's email, Log out
// logs this browser out and it stays out, and the email's code (from the dev server's log) logs it back in
// to the same account.
//   bun coax/web/dev-login-check.ts ORIGIN LOG EMAIL
// ORIGIN is a dev server (coax/web/serve.sh), LOG its terminal's output, EMAIL the account's email.
import { readFileSync } from "node:fs"
import { startChrome } from "./chrome.ts"

const [origin, log, email] = process.argv.slice(2)
if (!origin || !log || !email) { console.error("dev-login-check.ts ORIGIN LOG EMAIL"); process.exit(2) }
const { port } = await startChrome("oa-devlogin-")
const tab = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" })).json()
const ws = new WebSocket(tab.webSocketDebuggerUrl)
await new Promise((r) => ws.addEventListener("open", r, { once: true }))
let id = 0
const waiting = new Map<number, (v: any) => void>()
ws.addEventListener("message", (e) => { const m = JSON.parse(String(e.data)); waiting.get(m.id)?.(m); waiting.delete(m.id) })
const cdp = (method: string, params: object = {}) => new Promise<any>((r) => { const n = ++id; waiting.set(n, r); ws.send(JSON.stringify({ id: n, method, params })) })
const run = async (js: string) => (await cdp("Runtime.evaluate", { expression: js, awaitPromise: true, returnByValue: true })).result?.result?.value
const go = async (path: string) => { await cdp("Page.navigate", { url: origin + path }); await Bun.sleep(2500) }
const account = () => run(`fetch("/api/auth/account?as=lines").then(r => r.text())`)
const post = (path: string, body: object) => run(`fetch(${JSON.stringify(path)}, {method: "POST", headers: {"Content-Type": "application/json"}, body: ${JSON.stringify(JSON.stringify(body))}}).then(r => r.status + " " + r.text())`)
let failed = 0
const check = (what: string, ok: boolean, said = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok ? "" : ": " + said}`); if (!ok) failed++ }
const field = (lines: string, name: string) => (lines.match(new RegExp(`^${name} (.*)$`, "m")) ?? [])[1] ?? ""

await cdp("Page.enable")
await cdp("Emulation.setDeviceMetricsOverride", { width: 1400, height: 900, deviceScaleFactor: 1, mobile: false })
await go("/chat")
let a = await account()
const acct = field(a, "account")
check("this computer's request is the owner's account, named by its email", field(a, "primary") === email && acct.startsWith("acct_"), a)
check("the account is the admin key's too", /^key [0-9a-f]{64}$/m.test(a) && field(a, "admin") === "1", a)
const shot = (await cdp("Page.captureScreenshot", { format: "png" })).result?.data
if (shot && process.env.SHOT) await Bun.write(process.env.SHOT, Buffer.from(shot, "base64"))
check("Log out answers", (await post("/api/auth/logout?as=lines", {})).startsWith("200"))
check("logged out", field(await account(), "logged_in") === "0")
await go("/chat")
check("still logged out after a reload", field(await account(), "logged_in") === "0")
const before = readFileSync(log, "utf8").length
check("an email code is asked for", (await post("/api/auth/email/start?as=lines", { email })).startsWith("200"))
await Bun.sleep(1500)
const code = (readFileSync(log, "utf8").slice(before).match(new RegExp(`login code for ${email.replace(".", "\\.")}: (\\d{6})`)) ?? [])[1] ?? ""
check("the code shows in the dev server's terminal", code.length === 6)
check("the code logs in", (await post("/api/auth/email/verify?as=lines", { code })).startsWith("200"))
await go("/chat")
a = await account()
check("the same account, by its email, after the login", field(a, "account") === acct && field(a, "primary") === email && field(a, "admin") === "1", a)
check("Log out again", (await post("/api/auth/logout?as=lines", {})).startsWith("200"))
await go("/")
check("and it stays out", field(await account(), "logged_in") === "0")
ws.close()
process.exit(failed ? 1 : 0)
