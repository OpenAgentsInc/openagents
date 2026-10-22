// Starts The Haunted House: loads the wasm-bindgen glue beside this file,
// which fetches the module next to it and runs the house.
import init from "./haunt_web.js";

init().catch(function (error) {
  console.error("The house couldn't start. Reload the page to try again.", error);
});
