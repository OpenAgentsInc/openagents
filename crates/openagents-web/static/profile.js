// Settings > Public profile (#12254, src/profiles/edit.rs): when a Nostr
// browser extension (NIP-07) is installed, ask the server for the profile
// event to sign, have the extension sign it, and send it back to publish.
(function () {
  "use strict";
  var root = document.querySelector("[data-oa-profile-edit]");
  if (!root) return;
  var form = root.querySelector("[data-oa-profile-form]");
  var publish = root.querySelector("[data-oa-profile-publish]");
  var note = root.querySelector("[data-oa-profile-note]");
  var pubkey = root.getAttribute("data-pubkey");
  var ticket = {
    csrf: root.getAttribute("data-csrf"),
    request: root.getAttribute("data-request"),
  };

  function say(text) {
    if (note) note.textContent = text;
  }

  function post(path, body) {
    var payload = Object.assign({}, ticket, body);
    return fetch(path, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    }).then(function (response) {
      return response.json().catch(function () {
        return {};
      }).then(function (answer) {
        if (!response.ok) {
          throw new Error((answer.error && answer.error.message) || "That didn't work. Try again.");
        }
        return answer;
      });
    });
  }

  function fields() {
    var out = {};
    ["display_name", "picture", "about", "website"].forEach(function (name) {
      var input = form.elements.namedItem(name);
      out[name] = input ? input.value : "";
    });
    return out;
  }

  function ready() {
    if (!window.nostr || !publish) return false;
    publish.hidden = false;
    say("Your browser extension signs your profile with your key.");
    return true;
  }

  // Extensions often load after the page does.
  if (!ready()) setTimeout(ready, 600);

  form.addEventListener("submit", function (event) {
    event.preventDefault();
    if (!window.nostr) return;
    say("Getting your profile ready...");
    window.nostr
      .getPublicKey()
      .then(function (key) {
        if (key !== pubkey) {
          throw new Error("Your extension's key isn't the one on your account.");
        }
        return post("/settings/profile/draft", { fields: fields() });
      })
      .then(function (draft) {
        say("Sign it in your extension.");
        return window.nostr.signEvent(draft.event);
      })
      .then(function (signed) {
        say("Publishing...");
        return post("/settings/profile/publish", { event: signed });
      })
      .then(function (answer) {
        say("Published. ");
        var link = document.createElement("a");
        link.href = answer.href;
        link.textContent = "See your profile";
        note.appendChild(link);
      })
      .catch(function (error) {
        say(error.message || "That didn't work. Try again.");
      });
  });
})();
