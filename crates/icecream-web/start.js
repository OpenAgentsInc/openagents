// Starts The Ice Cream Man: loads the wasm-bindgen glue beside this file,
// which fetches the module next to it and runs the game. Before that, it
// makes sure the page never zooms, and on the website it registers the
// game's service worker so it can be installed and played offline.
import init from "./icecream_web.js";

// No zoom, ever. iOS Safari ignores the page's user-scalable=no, so a
// double tap or a pinch could zoom the game with no way back out. The
// game's controls set touch-action themselves; this stops the rest.
(function noZoom() {
  var stop = function (event) {
    if (event.cancelable) event.preventDefault();
  };
  var active = { passive: false };
  ["gesturestart", "gesturechange", "gestureend", "dblclick"].forEach(function (name) {
    document.addEventListener(name, stop, active);
  });
  // Two fingers moving together is a pinch, never a scroll here.
  document.addEventListener(
    "touchmove",
    function (event) {
      if (event.touches.length > 1 || (event.scale !== undefined && event.scale !== 1)) stop(event);
    },
    active,
  );
  // A second tap this soon after the first is never a double-tap zoom.
  var lastEnd = 0;
  document.addEventListener(
    "touchend",
    function (event) {
      var now = Date.now();
      if (now - lastEnd < 350 && event.touches.length === 0) stop(event);
      lastEnd = now;
    },
    active,
  );

  // And if the page is zoomed anyway, one big button puts it back.
  var view = window.visualViewport;
  if (!view) return;
  var meta = document.querySelector('meta[name="viewport"]');
  var original = meta ? meta.getAttribute("content") : "";
  var button = document.createElement("button");
  button.type = "button";
  button.id = "icecream-unzoom";
  button.textContent = "Zoom back out";
  var style = button.style;
  style.position = "fixed";
  style.zIndex = "2147483647";
  style.font = "800 18px ui-rounded, 'Trebuchet MS', system-ui, sans-serif";
  style.color = "#3a2418";
  style.background = "#ffd6e4";
  style.border = "3px solid #3a2418";
  style.borderRadius = "14px";
  style.padding = "10px 18px";
  style.boxShadow = "0 3px 0 #3a2418";
  style.transformOrigin = "0 0";
  style.touchAction = "manipulation";
  style.display = "none";
  var place = function () {
    var zoomed = view.scale > 1.02;
    style.display = zoomed ? "block" : "none";
    if (!zoomed) return;
    // Drawn at normal size in the corner of what you can see.
    style.left = view.offsetLeft + 12 / view.scale + "px";
    style.top = view.offsetTop + 12 / view.scale + "px";
    style.transform = "scale(" + 1 / view.scale + ")";
  };
  var unzoom = function (event) {
    if (event) event.preventDefault();
    if (meta) {
      // Changing the viewport makes the browser lay the page out again
      // at its scale of 1.
      meta.setAttribute("content", original.replace("initial-scale=1", "initial-scale=1.0001"));
      setTimeout(function () {
        meta.setAttribute("content", original);
        window.scrollTo(0, 0);
        place();
      }, 60);
    }
    window.scrollTo(0, 0);
  };
  button.addEventListener("click", unzoom);
  view.addEventListener("resize", place);
  view.addEventListener("scroll", place);
  var add = function () {
    document.body.appendChild(button);
    place();
  };
  if (document.body) add();
  else document.addEventListener("DOMContentLoaded", add);
})();

// On the website, the service worker keeps this build for offline play.
// The game's own app and a local test page skip it.
(function installable() {
  var game = "/games/the-ice-cream-man";
  if (!("serviceWorker" in navigator)) return;
  if (location.protocol !== "https:" && location.hostname !== "localhost" && location.hostname !== "127.0.0.1") return;
  if (location.pathname !== game && location.pathname.indexOf(game + "/") !== 0) return;
  var here = new URL(".", import.meta.url).href;
  var files = ["start.js", "icecream_web.js", "icecream_web_bg.wasm"].map(function (file) {
    return here + file;
  });
  files.push(
    location.origin + game,
    location.origin + game + "/manifest.webmanifest",
    location.origin + game + "/icon-180.png",
    location.origin + game + "/icon-192.png",
    location.origin + game + "/icon-512.png",
    "/static/legacy-demo.css",
    "/favicon.svg",
  );
  navigator.serviceWorker
    .register(game + "/sw.js", { scope: game })
    .then(function () {
      return navigator.serviceWorker.ready;
    })
    .then(function (registration) {
      if (registration.active) registration.active.postMessage({ cache: files });
    })
    .catch(function (error) {
      console.warn("The offline copy isn't available:", error);
    });
})();

init().catch(function (error) {
  var status = document.getElementById("icecream-status");
  if (status) {
    status.textContent = "The game couldn't start. Reload the page to try again.";
  }
  console.error(error);
});
