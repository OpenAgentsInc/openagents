// Starts the chat demo: loads the wasm-bindgen glue beside this file, which
// fetches the module next to it. GPUI then adds its canvas to the page, so
// the page's own content (the loading line) steps out of its way.
import init from "./openagents_web_demo.js";

init()
  .then(function () {
    var status = document.getElementById("gpui-status");
    if (status) {
      status.remove();
    }
    var content = document.getElementById("content");
    if (content) {
      content.style.display = "none";
    }
  })
  .catch(function (error) {
    var status = document.getElementById("gpui-status");
    if (status) {
      status.textContent = "The demo couldn't start. Reload the page to try again.";
    }
    console.error(error);
  });
