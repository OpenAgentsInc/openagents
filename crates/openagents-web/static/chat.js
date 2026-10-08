// The homepage and chat composer. Enter sends; Shift+Enter inserts a line.
// A click anywhere on the card that isn't a control focuses the text box.
// On a chat page the thread opens scrolled to the newest message.
(function () {
  "use strict";
  var thread = document.getElementById("chat-thread");
  if (thread) thread.scrollTop = thread.scrollHeight;
  var form = document.getElementById("chat-form");
  var input = document.getElementById("chat-input");
  if (!form || !input) return;
  input.addEventListener("keydown", function (event) {
    if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
    event.preventDefault();
    if (input.value.trim()) form.requestSubmit();
  });
  var card = document.getElementById("chat-card");
  if (card) {
    card.addEventListener("click", function (event) {
      if (event.target.closest("button, textarea")) return;
      input.focus();
    });
  }
})();
