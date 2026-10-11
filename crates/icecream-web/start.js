// Starts The Ice Cream Man: loads the wasm-bindgen glue beside this file,
// which fetches the module next to it and runs the game.
import init from "./icecream_web.js";

init().catch(function (error) {
  var status = document.getElementById("icecream-status");
  if (status) {
    status.textContent = "The game couldn't start. Reload the page to try again.";
  }
  console.error(error);
});
