"""WEB-17 browser acceptance for the Cloud workspace (openagents #10963).

Drives a fresh headless Chrome over CDP against the synthetic
`cloud_task_fixture` (loopback account service, resident host, one task):
IME, keyboard, sign-in reveal, narrow layouts, the task page's SSE observer,
a dropped observation stream, reload recovery, tab suspension, and native
session revocation. No real account, host, or credential is used.

  scripts/build-coder-cloud-web.sh BUILD   # or wasm-bindgen the debug build
  cargo run -p openagents-web --example cloud_task_fixture -- NEW_DIR BUILD 127.0.0.1:47917
  WEB17_RELAY=1 openagents browser run --browser bench/web/2026-10-08-web17/chrome-relay.sh -- \
    python3 bench/web/2026-10-08-web17/browser.py http://127.0.0.1:47917 ACCOUNT_SERVICE TASK_ROUTE

ACCOUNT_SERVICE and TASK_ROUTE are printed by the fixture. With WEB17_RELAY,
Chrome reaches the site through a loopback TCP relay on 127.0.0.1:47999
(chrome-relay.sh sets the proxy), so the driver can cut the observation
stream's connection the way a network drop would. Needs `websockets`.
"""
import asyncio, json, os, sys, time, urllib.request
import websockets

ORIGIN, ACCOUNT, TASK = sys.argv[1], sys.argv[2], sys.argv[3]
UPSTREAM = ("127.0.0.1", int(ORIGIN.rsplit(":", 1)[1]))
PORT = os.environ["OPENAGENTS_CHROME_PORT"]
results = {}


def record(name, ok, detail=""):
    results[name] = {"ok": bool(ok), "detail": detail}
    print(("PASS " if ok else "FAIL ") + name + (": " + str(detail) if detail else ""), flush=True)


def http(method, url, headers=None):
    req = urllib.request.Request(url, method=method, headers=headers or {})
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            return r.status, r.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


class Page:
    def __init__(self, ws):
        self.ws, self.n, self.pending, self.events = ws, 0, {}, []

    async def pump(self):
        async for raw in self.ws:
            msg = json.loads(raw)
            if "id" in msg and msg["id"] in self.pending:
                self.pending.pop(msg["id"]).set_result(msg)
            else:
                self.events.append(msg)

    async def call(self, method, **params):
        self.n += 1
        fut = asyncio.get_event_loop().create_future()
        self.pending[self.n] = fut
        await self.ws.send(json.dumps({"id": self.n, "method": method, "params": params}))
        msg = await asyncio.wait_for(fut, 30)
        if "error" in msg:
            raise RuntimeError(f"{method}: {msg['error']}")
        return msg["result"]

    async def eval(self, expr):
        r = await self.call("Runtime.evaluate", expression=expr, returnByValue=True, awaitPromise=True)
        return r["result"].get("value")

    async def until(self, expr, timeout=15):
        end = time.time() + timeout
        while time.time() < end:
            if await self.eval(expr):
                return True
            await asyncio.sleep(0.2)
        return False

    async def goto(self, url):
        await self.call("Page.navigate", url=url)
        await self.until("document.readyState === 'complete'", 20)

    async def key(self, key, code, vk):
        for t in ("keyDown", "keyUp"):
            await self.call("Input.dispatchKeyEvent", type=t, key=key, code=code, windowsVirtualKeyCode=vk)


REVEALED = "(() => { const p = document.getElementById('cloud-private'); return !!p && !p.hidden; })()"
RETIRED = "(() => { const p = document.getElementById('cloud-private'); const r = document.getElementById('cloud-resume'); return (!p || p.hidden) && !!r && !r.hidden; })()"
OVERFLOW = "document.documentElement.scrollWidth - window.innerWidth"


RELAY = []


async def relay_client(reader, writer):
    try:
        up_r, up_w = await asyncio.open_connection(*UPSTREAM)
    except OSError:
        writer.close(); return
    entry = [writer, up_w, False]
    RELAY.append(entry)

    async def pipe(a, b, sniff=False):
        try:
            while data := await a.read(65536):
                if sniff and data[:8].split(b" ")[0] in (b"GET", b"POST", b"PUT", b"DELETE", b"HEAD"):
                    entry[2] = b"/watch?" in data.split(b"\r\n", 1)[0]
                b.write(data); await b.drain()
        except Exception:
            pass
        finally:
            try: b.close()
            except Exception: pass
    await asyncio.gather(pipe(reader, up_w, True), pipe(up_r, writer))


def drop_all(only_watch=False):
    chosen = [e for e in RELAY if e[2] or not only_watch]
    for e in chosen:
        for x in (e[0], e[1]):
            try:
                x.transport.abort()
            except Exception:
                pass
        RELAY.remove(e)
    return len(chosen)


async def main():
    if os.environ.get("WEB17_RELAY"):
        await asyncio.start_server(relay_client, "127.0.0.1", 47999)
    status, body = http("PUT", f"http://127.0.0.1:{PORT}/json/new?about:blank")
    target = json.loads(body)
    async with websockets.connect(target["webSocketDebuggerUrl"], max_size=2**24) as ws:
        page = Page(ws)
        pump = asyncio.create_task(page.pump())
        for domain in ("Page", "Runtime", "Network"):
            await page.call(f"{domain}.enable")

        # Sign-in page: IME composition reaches the credential field.
        await page.goto(f"{ORIGIN}/cloud/sign-in")
        await page.eval("document.querySelector('input[name=credential]').focus()")
        await page.call("Input.imeSetComposition", text="にほ", selectionStart=2, selectionEnd=2)
        await page.call("Input.insertText", text="日本")
        ime = await page.eval("document.querySelector('input[name=credential]').value")
        record("ime_composition_commits_into_credential_field", ime == "日本", ime)

        # Keyboard: the first Tab reaches the skip link.
        await page.goto(f"{ORIGIN}/cloud/sign-in")
        await page.key("Tab", "Tab", 9)
        first = await page.eval("document.activeElement && document.activeElement.textContent.trim()")
        record("first_tab_reaches_skip_link", first == "Skip to content", first)

        await page.eval("""(() => { const f = document.querySelector('input[name=credential]');
            f.value = 'oak_alice.synthetic-credential'; f.form.submit(); })()""")
        t0 = time.time()
        ok = await page.until(REVEALED, 20)
        record("sign_in_reveals_workspace_after_standing_check", ok, f"{time.time()-t0:.2f}s")

        # Select the personal workspace through its keyboard-reachable button.
        await page.eval("""[...document.querySelectorAll('.cloud-switcher button')]
            .find(b => b.textContent.includes('Alice personal')).click()""")
        await asyncio.sleep(1)
        ok = await page.until(REVEALED, 20)
        current = await page.eval("document.querySelector('nav[aria-label=Workspace] [aria-current=page]')?.textContent")
        record("workspace_selected_and_current_section_marked", ok and current == "Overview", current)

        # Narrow layouts: no horizontal scroll at phone widths.
        for width in (320, 375):
            await page.call("Emulation.setDeviceMetricsOverride", width=width, height=740, deviceScaleFactor=2, mobile=True)
            for path in ("/cloud/app", TASK):
                await page.goto(ORIGIN + path)
                await page.until(REVEALED, 20)
                over = await page.eval(OVERFLOW)
                record(f"no_horizontal_scroll_{width}px_{path.split('/')[-1] or 'app'}", over is not None and over <= 0, f"overflow {over}px")
        await page.call("Emulation.clearDeviceMetricsOverride")

        # Task page: SSE observation, then a dropped connection reconnects with Last-Event-ID.
        page.events.clear()
        await page.goto(ORIGIN + TASK)
        ok = await page.until(REVEALED, 20)
        has_observer = await page.eval("!!document.querySelector('#cloud-observation[sse-connect]')")
        record("task_page_revealed_with_sse_observer", ok and has_observer)

        await page.until("true", 1)
        end = time.time() + 15
        while time.time() < end and not any(e.get("method") == "Network.eventSourceMessageReceived" for e in page.events):
            await asyncio.sleep(0.2)
        msgs = [e["params"] for e in page.events if e.get("method") == "Network.eventSourceMessageReceived"]
        record("first_sse_event_names_snapshot", bool(msgs) and msgs[0]["eventName"] == "refresh" and msgs[0]["eventId"].startswith("v1:"),
               msgs[0]["eventName"] + " " + msgs[0]["eventId"][:20] if msgs else "none")
        if os.environ.get("WEB17_RELAY"):
            dropped = drop_all(only_watch=True)
            print("dropped", dropped, "watch stream connections", flush=True)
        else:
            await page.call("Network.emulateNetworkConditions", offline=True, latency=0, downloadThroughput=-1, uploadThroughput=-1)
            await asyncio.sleep(3)
            await page.call("Network.emulateNetworkConditions", offline=False, latency=0, downloadThroughput=-1, uploadThroughput=-1)
        t0 = time.time()
        ok = await page.until(RETIRED, 15)
        resume = await page.eval("document.querySelector('#cloud-resume a[href=\\'/cloud/app\\']')?.textContent")
        leftover = await page.eval("document.body.innerText.includes('Original private request')")
        record("dropped_sse_retires_view_instead_of_looking_current", ok and not leftover and resume,
               f"{time.time()-t0:.1f}s; resume link: {resume}")
        page.events.clear()
        await page.call("Page.reload")
        await asyncio.sleep(0.5)
        ok = await page.until(REVEALED, 20)
        end = time.time() + 15
        while time.time() < end and not any(e.get("method") == "Network.eventSourceMessageReceived" for e in page.events):
            await asyncio.sleep(0.2)
        msgs = [e["params"] for e in page.events if e.get("method") == "Network.eventSourceMessageReceived"]
        record("reload_after_drop_readmits_and_observes_current_snapshot",
               ok and bool(msgs) and msgs[0]["eventName"] == "refresh",
               msgs[0]["eventName"] if msgs else "none")

        # Refresh recovery: a reload re-admits and shows the same task.
        await page.call("Page.reload")
        await asyncio.sleep(0.5)
        ok = await page.until(REVEALED, 20)
        same = await page.eval("location.pathname") == TASK
        record("reload_recovers_same_task_view", ok and same)

        # Tab suspension: another tab takes the foreground.
        status, body = http("PUT", f"http://127.0.0.1:{PORT}/json/new?about:blank")
        other = json.loads(body)
        http("GET", f"http://127.0.0.1:{PORT}/json/activate/{other['id']}")
        await asyncio.sleep(1)
        vis = await page.eval("document.visibilityState")
        if vis != "hidden":
            # Headless keeps every tab visible; emulate the page lifecycle instead.
            await page.call("Page.setWebLifecycleState", state="frozen")
            await page.call("Page.setWebLifecycleState", state="active")
            vis2 = await page.eval("document.visibilityState")
            if not await page.eval(RETIRED):
                await page.eval("""(() => { Object.defineProperty(document, 'visibilityState', {configurable: true, get: () => 'hidden'});
                    Object.defineProperty(document, 'hidden', {configurable: true, get: () => true});
                    document.dispatchEvent(new Event('visibilitychange')); })()""")
                method = f"synthetic visibilitychange (headless visibility stayed {vis}/{vis2})"
            else:
                method = "page lifecycle freeze"
        else:
            method = "real background tab"
        ok = await page.until(RETIRED, 10)
        leftover = await page.eval("document.body.innerText.includes('Original private request')")
        record("tab_suspension_retires_private_content", ok and not leftover, method)
        http("GET", f"http://127.0.0.1:{PORT}/json/close/{other['id']}")
        await page.call("Page.reload")
        await asyncio.sleep(0.5)
        ok = await page.until(REVEALED, 20)
        record("return_after_suspension_needs_navigation_and_readmits", ok)

        # Session expiry: revoke the native session; the open page retires itself.
        status, _ = http("DELETE", f"{ACCOUNT}/v1/session", {"Authorization": "Bearer sess_" + "a" * 64})
        t0 = time.time()
        ok = await page.until(RETIRED, 30)
        leftover = await page.eval("document.body.innerText.includes('Original private request')")
        record("revoked_session_retires_open_page", status == 200 and ok and not leftover, f"{time.time()-t0:.1f}s after revoke")
        await page.goto(ORIGIN + "/cloud/app")
        signin = await page.eval("location.pathname")
        record("reopen_after_expiry_requires_sign_in", signin != "/cloud/app" or not await page.eval(REVEALED), signin)
        pump.cancel()
    print(json.dumps(results))
    sys.exit(0 if all(r["ok"] for r in results.values()) else 1)


asyncio.run(main())
