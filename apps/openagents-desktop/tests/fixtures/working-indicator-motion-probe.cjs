const { app, BrowserWindow } = require("electron")
const { readFileSync } = require("node:fs")
const path = require("node:path")

const cssPath = path.resolve(__dirname, "../../../../packages/ui/src/desktop-workbench.css")
const css = readFileSync(cssPath, "utf8")

const readAnimationState = window => window.webContents.executeJavaScript(`(() => {
  const indicator = document.querySelector('.oa-react-working')
  const bars = [...indicator.querySelectorAll('i')]
  return {
    label: indicator.getAttribute('aria-label'),
    animations: bars.map(bar => {
      const computed = getComputedStyle(bar)
      const animation = bar.getAnimations()[0]
      return {
        animationName: computed.animationName,
        playState: animation?.playState ?? null,
        currentTime: typeof animation?.currentTime === 'number' ? animation.currentTime : null,
      }
    }),
  }
})()`)

const main = async () => {
  await app.whenReady()
  const window = new BrowserWindow({
    show: false,
    webPreferences: {
      backgroundThrottling: false,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  })
  const html = `<!doctype html><style>${css}</style><div class="oa-react-working" role="status" aria-label="Codex is working"><span>Working</span><i></i><i></i><i></i></div>`
  await window.loadURL(`data:text/html;charset=utf-8,${encodeURIComponent(html)}`)
  const first = await readAnimationState(window)
  await new Promise(resolve => setTimeout(resolve, 160))
  const second = await readAnimationState(window)

  await window.webContents.debugger.attach("1.3")
  await window.webContents.debugger.sendCommand("Emulation.setEmulatedMedia", {
    media: "screen",
    features: [{ name: "prefers-reduced-motion", value: "reduce" }],
  })
  await window.loadURL(`data:text/html;charset=utf-8,${encodeURIComponent(html)}`)
  const reduced = await readAnimationState(window)
  window.webContents.debugger.detach()

  process.stdout.write(`${JSON.stringify({ first, second, reduced })}\n`)
  window.destroy()
  app.quit()
}

main().catch(error => {
  process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`)
  app.exit(1)
})
