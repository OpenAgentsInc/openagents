// Watches a framed game (#12478): the card's preview, Play on the card,
// and Play over the Grid. A frame that shows a browser error page, or
// that hasn't loaded in time, is covered by a dark note with Try again and
// Back to the Grid, never a broken or white frame. The game's start script
// posts `openagents-game-ready` once it draws and `openagents-game-failed`
// when it can't start; a frame whose page loaded with the game's canvas
// counts as loaded too, so an older build still passes.
globalThis.openagentsWatchGameFrame = function (frame, back) {
  if (frame.__oaWatch) return frame.__oaWatch;
  var box = frame.parentElement;
  var note = document.createElement('div');
  note.className = 'oa-game-failed';
  note.setAttribute('role', 'alert');
  note.hidden = true;
  note.style.cssText =
    'position:absolute;inset:0;z-index:23;display:flex;flex-direction:column;align-items:center;' +
    'justify-content:center;gap:12px;background:#101010;color:#eee;font:15px system-ui,sans-serif;text-align:center';
  var said = document.createElement('p');
  said.style.margin = '0';
  said.textContent = "The game didn't load.";
  var row = document.createElement('div');
  row.style.cssText = 'display:flex;gap:8px;flex-wrap:wrap;justify-content:center';
  var buttonStyle =
    'border:1px solid #555;border-radius:8px;background:#222;color:#eee;padding:8px 14px;cursor:pointer;font:inherit;text-decoration:none';
  var retry = document.createElement('button');
  retry.type = 'button';
  retry.textContent = 'Try again';
  retry.style.cssText = buttonStyle;
  var home = document.createElement(back ? 'button' : 'a');
  home.textContent = 'Back to the Grid';
  home.style.cssText = buttonStyle;
  if (back) {
    home.type = 'button';
    home.addEventListener('click', back);
  } else {
    home.href = '/';
    home.target = '_top';
  }
  row.appendChild(retry);
  row.appendChild(home);
  note.appendChild(said);
  note.appendChild(row);
  box.appendChild(note);
  // Show the note in place of the frame (the note's own `display:flex`
  // would otherwise beat `hidden`).
  var show = function (on) {
    note.hidden = !on;
    note.style.display = on ? 'flex' : 'none';
    frame.style.visibility = on ? 'hidden' : '';
  };
  show(false);
  var timer = null;
  var fine = false;
  var fail = function () {
    if (fine) return;
    clearTimeout(timer);
    show(true);
  };
  var good = function () {
    fine = true;
    clearTimeout(timer);
    show(false);
  };
  var hasGame = function () {
    try {
      var doc = frame.contentDocument;
      return !!(doc && doc.getElementById('icecream-canvas'));
    } catch (_) {
      return false;
    }
  };
  var watch = {
    start: function () {
      fine = false;
      show(false);
      clearTimeout(timer);
      if (frame.getAttribute('src')) timer = setTimeout(fail, 30000);
    },
    stop: function () {
      fine = true;
      clearTimeout(timer);
      show(false);
    },
  };
  frame.addEventListener('load', function () {
    var src = frame.getAttribute('src');
    if (!src || src === 'about:blank') return;
    if (hasGame()) good();
    else fail();
  });
  addEventListener('message', function (event) {
    if (event.source !== frame.contentWindow || event.origin !== location.origin) return;
    var type = event.data && event.data.type;
    if (type === 'openagents-game-ready') good();
    if (type === 'openagents-game-failed') {
      fine = false;
      fail();
    }
  });
  retry.addEventListener('click', function () {
    var src = frame.getAttribute('src');
    if (!src) return;
    watch.start();
    frame.setAttribute('src', 'about:blank');
    setTimeout(function () {
      frame.setAttribute('src', src);
    }, 0);
  });
  frame.__oaWatch = watch;
  if (hasGame()) good();
  else watch.start();
  return watch;
};
