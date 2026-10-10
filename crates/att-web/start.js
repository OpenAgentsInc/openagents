// Starts the sealed-inference demo: loads the wasm-bindgen glue beside this
// file, which fetches the module next to it and mounts the page.
import init from "./att_web.js";

init().catch(function (error) {
  var status = document.getElementById("att-status");
  if (status) {
    status.textContent = "The demo couldn't start. Reload the page to try again.";
  }
  console.error(error);
});
