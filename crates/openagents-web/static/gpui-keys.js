// Keep the HTML settings page unless this browser can draw the credentials module.
const page = location.pathname.split("/").pop();
if (["api-keys", "sign-in-keys", "agent-accounts"].includes(page)) {
  try {
    if (navigator.gpu && await navigator.gpu.requestAdapter()) {
      location.replace("/gpui?ui=gpui&settings=" + page);
    }
  } catch (_) {
    // The HTML form stays available.
  }
}
