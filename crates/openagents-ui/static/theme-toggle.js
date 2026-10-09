// Theme toggle for openagents-ui (adoption plan, decision 6).
//
// Contract with openagents_ui::shell::ThemeToggle:
// - binds to [data-oa-theme-toggle] (THEME_TOGGLE_ATTR);
// - a click flips <html data-theme> to the opposite of the effective theme
//   (no data-theme means the system setting, via prefers-color-scheme);
// - stores the choice in the oa_theme cookie (THEME_COOKIE) as light|dark,
//   Path=/, SameSite=Lax, one year, so the server paints the next page in
//   that theme with no flash; clearing it returns to the system setting;
// - when the button sits in its no-JS fallback form (POST /theme with
//   theme=toggle), the submit is prevented and the switch happens in place.
//
// CSP-safe: no eval, no inline handlers, no string timers. Needs no Alpine.
(function () {
  "use strict";
  var COOKIE = "oa_theme";
  var MAX_AGE = 60 * 60 * 24 * 365;
  var root = document.documentElement;

  function effective() {
    var chosen = root.getAttribute("data-theme");
    if (chosen === "light" || chosen === "dark") return chosen;
    var dark = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
    return dark ? "dark" : "light";
  }

  function choose(theme) {
    root.setAttribute("data-theme", theme);
    var secure = window.location.protocol === "https:" ? "; Secure" : "";
    document.cookie = COOKIE + "=" + theme + "; Path=/; Max-Age=" + MAX_AGE + "; SameSite=Lax" + secure;
  }

  document.addEventListener("click", function (event) {
    var target = event.target;
    var button = target && target.closest ? target.closest("[data-oa-theme-toggle]") : null;
    if (!button) return;
    event.preventDefault();
    choose(effective() === "dark" ? "light" : "dark");
  });
})();
