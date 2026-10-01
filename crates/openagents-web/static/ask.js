// The homepage terminal (#10106). `help`, `install`, `docs`, and `clear`
// are its commands, matched whole; any other line is a question posted to
// /ask, whose answer streams back as newline-delimited JSON: {"html"} as it
// grows (drawn by the server, raw HTML shown as text), then {"done","text"}
// or {"error"}. The conversation lives in this page only.
(function () {
  "use strict";
  var screen = document.getElementById("term-screen");
  var form = document.getElementById("term-form");
  var input = document.getElementById("term-input");
  if (!screen || !form || !input || !window.fetch) return;
  var turns = [];
  var busy = false;

  function row(cls, text) {
    var p = document.createElement("p");
    p.className = cls;
    if (text !== undefined) p.textContent = text;
    screen.appendChild(p);
    screen.scrollTop = screen.scrollHeight;
    return p;
  }

  function said(text) {
    var p = row("term-said");
    var mark = document.createElement("span");
    mark.className = "term-mark";
    mark.textContent = "> ";
    p.appendChild(mark);
    p.appendChild(document.createTextNode(text));
  }

  function linked(before, href, label) {
    var p = row("term-out", before);
    var a = document.createElement("a");
    a.href = href;
    a.textContent = label;
    p.appendChild(a);
  }

  var commands = {
    help: function () {
      row("term-out", "help     what you can type here");
      row("term-out", "install  get OpenAgents for Mac and iPhone");
      row("term-out", "docs     guides to the apps and plugins");
      row("term-out", "clear    empty the screen");
      row("term-out", "Anything else is a question for OpenAgents.");
    },
    install: function () {
      linked("OpenAgents for Mac, and for iPhone beside it: ", "/install", "openagents.com/install");
    },
    docs: function () {
      linked("Guides to the apps and plugins: ", "/docs", "openagents.com/docs");
    },
    clear: function () {
      screen.textContent = "";
    }
  };

  function finish() {
    busy = false;
    input.disabled = false;
    input.focus();
  }

  function ask(question) {
    turns.push({ role: "user", text: question });
    turns = turns.slice(-8);
    var answer = document.createElement("div");
    answer.className = "term-answer";
    answer.textContent = "…";
    screen.appendChild(answer);
    screen.scrollTop = screen.scrollHeight;
    busy = true;
    input.disabled = true;
    var failed = function (message) {
      answer.className = "term-answer term-error";
      answer.textContent = message;
      turns.pop();
      finish();
    };
    fetch("/ask", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ turns: turns })
    }).then(function (response) {
      if (!response.ok || !response.body) {
        return response.json().then(function (body) {
          failed(body && body.error ? body.error : "We can't answer right now; try again soon.");
        }, function () {
          failed("We can't answer right now; try again soon.");
        });
      }
      var reader = response.body.getReader();
      var decoder = new TextDecoder();
      var pending = "";
      var ended = false;
      var take = function (text) {
        var message;
        try { message = JSON.parse(text); } catch (e) { return; }
        if (typeof message.html === "string") {
          answer.innerHTML = message.html;
          screen.scrollTop = screen.scrollHeight;
        }
        if (message.error) {
          ended = true;
          failed(message.error);
        } else if (message.done) {
          ended = true;
          turns.push({ role: "assistant", text: message.text || "" });
          finish();
        }
      };
      var pump = function () {
        return reader.read().then(function (chunk) {
          if (chunk.done) {
            if (pending) take(pending);
            if (!ended) failed("We didn't get an answer; try asking again.");
            return;
          }
          pending += decoder.decode(chunk.value, { stream: true });
          var lines = pending.split("\n");
          pending = lines.pop();
          lines.forEach(take);
          return pump();
        });
      };
      return pump();
    }).catch(function () {
      failed("We can't reach OpenAgents right now; try again soon.");
    });
  }

  form.addEventListener("submit", function (event) {
    event.preventDefault();
    if (busy) return;
    var line = input.value.trim();
    if (!line) return;
    input.value = "";
    said(line);
    var command = commands[line.toLowerCase()];
    if (command && Object.prototype.hasOwnProperty.call(commands, line.toLowerCase())) {
      command();
      screen.scrollTop = screen.scrollHeight;
    } else {
      ask(line);
    }
  });
})();
