// Presentation glue for Game Lab (#12459). Verse opens it under its arch
// (`homeGameLab(true)` on E or a tap there) and closes it when the visitor
// walks away; the GPUI chat draws the panel, checks sign-in, reads the
// card, and records the visit's Game Lab events. This shows a game full
// screen and keeps the panel's hidden section current for screen readers.
const gamePanel = document.getElementById('game-lab-panel');
const gamePlay = document.getElementById('game-lab-play');
const gameFrame = document.getElementById('game-lab-play-frame');
const gameWatch = openagentsWatchGameFrame(gameFrame, () => {
  homeGamePlay(null);
  homeGameLab(false);
});
globalThis.homeGamePlay = src => {
  globalThis.openagentsGamePlaying = !!src;
  document.documentElement.classList.toggle('game-playing', !!src);
  if (src) {
    gameFrame.src = src;
    gamePlay.hidden = false;
    gameWatch.start();
  } else {
    gamePlay.hidden = true;
    gameWatch.stop();
    gameFrame.removeAttribute('src');
  }
};
globalThis.homeGameLab = open => {
  open = !!open && world.classList.contains('ready');
  if (open === !!globalThis.openagentsGameLabOpen) return;
  globalThis.openagentsGameLabOpen = open;
  gamePanel.hidden = !open;
  if (open) {
    loadChat().catch(() => {});
    return;
  }
  homeGamePlay(null);
  world.focus({preventScroll: true});
};
document.getElementById('game-lab-close').addEventListener('click', () => homeGameLab(false));
document.getElementById('game-lab-back').addEventListener('click', () => {
  homeGamePlay(null);
  homeTerminal(true);
});
const gameEscape = event => {
  if (event.key !== 'Escape' || !globalThis.openagentsGameLabOpen) return;
  event.preventDefault();
  event.stopImmediatePropagation();
  homeGameLab(false);
};
addEventListener('keydown', gameEscape, true);
gameFrame.addEventListener('load', () => {
  try { gameFrame.contentWindow.addEventListener('keydown', gameEscape, true); } catch (_) {}
  if (!gamePlay.hidden) gameFrame.focus();
});
if (new URLSearchParams(location.search).get('lab') === 'game') {
  const openWhenReady = new MutationObserver(() => {
    if (!world.classList.contains('ready')) return;
    openWhenReady.disconnect();
    homeGameLab(true);
  });
  openWhenReady.observe(world, {attributes: true, attributeFilter: ['class']});
}
// Sign-in keeps the visitor in the world (#12480): GitHub opens in a
// popup, which ends on `/auth/done` and tells this page; the Grid stays
// loaded and Game Lab opens signed in where they stand. With popups
// blocked, the full-page sign-in comes back to `fallback`'s address.
globalThis.openagentsSignIns = 0;
globalThis.homeSignIn = fallback => {
  const popup = open('/auth/popup', 'openagents-sign-in', 'popup,width=520,height=720');
  if (!popup) location.assign(fallback);
};
let signedInAt = -1e9;
const signedIn = data => {
  if (!data || data.type !== 'openagents-signed-in') return;
  // The popup says it on both paths; count it once.
  if (performance.now() - signedInAt < 2000) return;
  signedInAt = performance.now();
  globalThis.openagentsSignIns += 1;
  account();
};
addEventListener('message', event => {
  if (event.origin === location.origin) signedIn(event.data);
});
try {
  new BroadcastChannel('openagents-sign-in').onmessage = event => signedIn(event.data);
} catch (_) {}
document.getElementById('game-lab-signin').addEventListener('click', event => {
  event.preventDefault();
  homeSignIn(event.currentTarget.getAttribute('href'));
});
