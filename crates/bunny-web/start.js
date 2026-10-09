// Starts Grow Little Bunny: loads the wasm-bindgen glue beside this file,
// which fetches the module next to it and runs the game.
import init from "./bunny_web.js";

init().catch(function (error) {
  var status = document.getElementById("bunny-status");
  if (status) {
    status.textContent = "The game couldn't start. Reload the page to try again.";
  }
  console.error(error);
});
