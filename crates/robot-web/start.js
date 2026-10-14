// Start the Rust player. Generated wasm-bindgen glue owns the browser boundary.
import init, { start } from "./robot_web.js";
init().then(start).catch(function(error) {
  const status = document.getElementById("status");
  if (status) status.textContent = "The robot view couldn't start. Reload to try again.";
  console.error(error);
});
