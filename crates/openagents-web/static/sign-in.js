// Log in with a Nostr key on /login (#12001, src/web_sign_in.rs): show a
// code the OpenAgents phone app scans, wait for the phone to approve, and
// offer a browser extension (NIP-07) when one is installed.
(function () {
  "use strict";
  var root = document.querySelector("[data-oa-web-sign-in]");
  if (!root) return;
  var back = root.getAttribute("data-return-to") || "/";
  var img = root.querySelector("[data-oa-web-sign-in-qr]");
  var note = root.querySelector("[data-oa-web-sign-in-note]");
  var extension = root.querySelector("[data-oa-web-sign-in-extension]");
  var button = root.querySelector("[data-oa-web-sign-in-nip07]");
  var codes = 0;
  var timer = null;

  function say(text) {
    if (note) note.textContent = text;
  }

  function done(answer) {
    if (timer) clearTimeout(timer);
    say("You're logged in. Taking you back now.");
    window.location.assign(answer.return_to || back);
  }

  function json(response) {
    return response.json().catch(function () {
      return {};
    }).then(function (body) {
      return { ok: response.ok, status: response.status, body: body };
    });
  }

  function message(body, fallback) {
    return (body && body.error && body.error.message) || fallback;
  }

  function poll(request, interval) {
    timer = setTimeout(function () {
      fetch("/login/key/" + request + "/status", { credentials: "same-origin" })
        .then(json)
        .then(function (answer) {
          if (answer.ok && answer.body.status === "signed_in") return done(answer.body);
          if (answer.ok) return poll(request, interval);
          if (answer.status === 404) return start();
          say("Login isn't available right now. Reload the page to try again.");
        })
        .catch(function () {
          poll(request, interval * 2);
        });
    }, interval * 1000);
  }

  function start() {
    codes += 1;
    if (codes > 6) {
      if (img) img.hidden = true;
      say("Your code expired. Reload the page for a new one.");
      return;
    }
    fetch("/login/key", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ return_to: back }),
    })
      .then(json)
      .then(function (answer) {
        if (!answer.ok) {
          say(message(answer.body, "Scanning isn't available right now."));
          return;
        }
        if (img) {
          img.src = answer.body.qr;
          img.hidden = false;
        }
        say("Open the OpenAgents app on your phone and scan this code.");
        poll(answer.body.request, answer.body.interval || 2);
      })
      .catch(function () {
        say("Scanning isn't available right now.");
      });
  }

  function base64(text) {
    var bytes = new TextEncoder().encode(text);
    var binary = "";
    for (var i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary);
  }

  function sha256(text) {
    return crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)).then(function (digest) {
      return Array.prototype.map
        .call(new Uint8Array(digest), function (b) {
          return ("0" + b.toString(16)).slice(-2);
        })
        .join("");
    });
  }

  function withExtension() {
    var url = window.location.origin + "/login/key/extension";
    var body = JSON.stringify({ return_to: back });
    if (button) button.disabled = true;
    say("Approve the login in your extension.");
    var again = function (text) {
      if (button) button.disabled = false;
      say(text);
    };
    fetch("/v1/device/nostr/challenge", { credentials: "same-origin" })
      .then(json)
      .then(function (answer) {
        if (!answer.ok || !answer.body.challenge) throw new Error("challenge");
        return sha256(body).then(function (hash) {
          return window.nostr.signEvent({
            kind: 27235,
            created_at: Math.floor(Date.now() / 1000),
            tags: [
              ["u", url],
              ["method", "POST"],
              ["challenge", answer.body.challenge],
              ["payload", hash],
            ],
            content: "",
          });
        });
      })
      .then(function (event) {
        return fetch(url, {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "Content-Type": "application/json",
            Authorization: "Nostr " + base64(JSON.stringify(event)),
          },
          body: body,
        }).then(json);
      })
      .then(function (answer) {
        if (answer.ok && answer.body.status === "signed_in") return done(answer.body);
        again(message(answer.body, "Your extension's login didn't go through. Try again."));
      })
      .catch(function () {
        again("Your extension didn't sign the login. Try again.");
      });
  }

  function offerExtension() {
    if (!extension || !window.nostr || typeof window.nostr.signEvent !== "function") return;
    if (!window.crypto || !window.crypto.subtle) return;
    extension.hidden = false;
  }

  if (button) button.addEventListener("click", withExtension);
  offerExtension();
  // Extensions can add window.nostr after the page loads.
  setTimeout(offerExtension, 800);
  start();
})();
