// Starts the LayerTree demo: loads the wasm-bindgen glue beside this file,
// which fetches the module next to it and draws the scene in the page's
// #layertree-canvas.
import init from "./layertree_web.js";

init().catch(function (error) {
  var status = document.getElementById("layertree-status");
  if (status) {
    status.textContent = "The LayerTree demo couldn't start. Reload the page to try again.";
  }
  console.error(error);
});
