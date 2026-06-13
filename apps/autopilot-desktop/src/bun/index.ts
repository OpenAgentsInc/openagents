import { BrowserWindow } from "electrobun/bun";

const window = new BrowserWindow({
  title: "Autopilot Desktop",
  url: "views://autopilot-desktop/index.html"
});

// TODO: Wire the Pylon control client over loopback (apps/pylon control-client)
// and expose typed RPC to the webview.
void window;
