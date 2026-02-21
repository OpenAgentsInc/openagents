import init from "./openagents_web_shell.js";

const STATUS_ID = "openagents-web-shell-status";

function setStatus(text, isError = false) {
  const status = document.getElementById(STATUS_ID);
  if (!status) return;
  status.textContent = text;
  status.style.color = isError ? "#f87171" : "#cbd5e1";
}

function registerServiceWorker() {
  if (!("serviceWorker" in navigator)) return;
  navigator.serviceWorker
    .register("/sw.js")
    .catch((error) => console.warn("service worker registration failed", error));
}

async function boot() {
  setStatus("Boot: loading wasm entrypoint");
  registerServiceWorker();
  await init("./openagents_web_shell_bg.wasm");
  setStatus("Boot: wasm initialized");
}

boot().catch((error) => {
  setStatus(`Boot error: ${String(error)}`, true);
  console.error("openagents web shell bootstrap failed", error);
});
