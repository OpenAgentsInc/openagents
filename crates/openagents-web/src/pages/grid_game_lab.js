// Presentation glue for the Rust Game Lab entrance and the shared chat.
const gamePanel = document.getElementById('game-lab-panel');
const gameCard = document.getElementById('game-lab-card');
const gameStatus = document.getElementById('game-lab-status');
const gameSignIn = document.getElementById('game-lab-signin');
const gamePlay = document.getElementById('game-lab-play');
const gameFrame = document.getElementById('game-lab-play-frame');
let gameOpen = false, gameNear = false, gameSuppressed = false;
// The visit's Game Lab events (#12439): what was done, never what was typed.
let gameOpenedAt = 0, gameChecking = false;
const gameSeen = new Set();
const setGameOpen = async open => {
  if (!open) {
    if (gameOpen) track('game_lab_closed', {time: Math.round(performance.now() - gameOpenedAt)});
    gameOpen = false;
    globalThis.openagentsLab = undefined;
    gamePanel.hidden = true;
    gameCard.hidden = true;
    gameCard.removeAttribute('src');
    gamePlay.hidden = true;
    gameFrame.removeAttribute('src');
    homeTerminal(false);
    world.focus({preventScroll:true});
    return;
  }
  if (gameOpen || gamePanel.hasAttribute('data-off')) return;
  gameOpen = true;
  gameOpenedAt = performance.now();
  gamePanel.hidden = false;
  gameStatus.textContent = 'Opening Game Lab…';
  gameSignIn.hidden = true;
  try {
    const response = await fetch('/labs/game/session');
    if (!gameOpen) return;
    if (response.status === 401 || response.status === 403) {
      gameStatus.textContent = 'Sign in to make your own version.';
      gameSignIn.hidden = false;
      track('game_lab_opened', {result: 'sign_in'});
      return;
    }
    if (!response.ok) throw new Error('Game Lab could not open');
    globalThis.openagentsLab = 'game';
    gameStatus.textContent = 'Tell us what you want to change.';
    gameCard.src = '/labs/game/card';
    gameCard.hidden = false;
    homeTerminal(true);
    track('game_lab_opened', {result: 'open'});
  } catch (_) {
    track('game_lab_opened', {result: 'failed'});
    gameStatus.textContent = 'Game Lab couldn’t open. Close it and try again.';
  }
};
globalThis.homeGameLab = near => {
  gameNear = near;
  if (!near) gameSuppressed = false;
  if (near && !gameSuppressed) setGameOpen(true);
};
globalThis.homeGameCards = data => {
  if (!gameOpen) return;
  try {
    const value = JSON.parse(data);
    const cards = Array.isArray(value) ? value : value.cards || [];
    const card = cards.find(card => card.card === 'game_lab' && card._hosted);
    if (!card) return;
    if (card.compare) track('game_lab_compared');
    else if (gameChecking && !card.autoplay && !gameSeen.has(card.version)) track('game_lab_version_saved');
    gameSeen.add(card.version);
    const query = new URLSearchParams({v:String(card.version)});
    if (card.compare) query.set('a',String(card.compare.a.version));
    gameCard.src = '/labs/game/card?' + query;
    gameStatus.textContent = card.summary || '';
    if (card.autoplay) {
      gameFrame.src = '/games/the-ice-cream-man#' + card.fragment + '&skip';
      gamePlay.hidden = false; homeTerminal(false);
      track('game_lab_played', {from: 'chat'});
    }
  } catch (_) {}
};
document.getElementById('game-lab-open').addEventListener('click',event => {
  event.preventDefault(); gameSuppressed = false; setGameOpen(true);
});
document.getElementById('game-lab-close').addEventListener('click',() => {
  gameSuppressed = true; setGameOpen(false);
});
document.getElementById('game-lab-back').addEventListener('click',() => {
  gamePlay.hidden = true; gameFrame.removeAttribute('src'); homeTerminal(true);
});
addEventListener('message',event => {
  if (event.origin !== location.origin || event.source !== gameCard.contentWindow || event.data?.type !== 'game-fullscreen') return;
  const url = new URL(event.data.src,location.origin);
  if (url.origin !== location.origin || url.pathname !== '/games/the-ice-cream-man') return;
  gameFrame.src = url.href; gamePlay.hidden = false; homeTerminal(false);
  track('game_lab_played', {from: 'card'});
});
const gameEscape = event => {
  if (event.key !== 'Escape' || !gameOpen) return;
  event.preventDefault(); event.stopImmediatePropagation();
  gameSuppressed = true; setGameOpen(false);
};
addEventListener('keydown',gameEscape,true);
const bindGameEscape = frame => {
  try { frame.contentWindow.addEventListener('keydown',gameEscape,true); } catch (_) {}
};
gameFrame.addEventListener('load',() => {
  bindGameEscape(gameFrame);
  if (!gamePlay.hidden) gameFrame.focus();
});
gameCard.addEventListener('load',() => {
  bindGameEscape(gameCard);
  for (const frame of gameCard.contentDocument?.querySelectorAll('iframe') || []) {
    bindGameEscape(frame);
    frame.addEventListener('load',() => bindGameEscape(frame));
  }
});
new MutationObserver(() => {
  const checking = composer.dataset.chatState === 'streaming' || composer.dataset.chatState === 'sending';
  if (gameOpen && globalThis.openagentsLab === 'game') {
    if (checking && !gameChecking) track('game_lab_check');
    gameChecking = checking;
  }
  if (gameOpen && globalThis.openagentsLab === 'game') gameStatus.textContent =
    checking ? 'Checking…' : 'Tell us what you want to change.';
}).observe(composer,{attributes:true,attributeFilter:['data-chat-state']});
if (new URLSearchParams(location.search).get('lab') === 'game') {
  const openWhenReady = new MutationObserver(() => {
    if (!world.classList.contains('ready')) return;
    openWhenReady.disconnect(); setGameOpen(true);
  });
  openWhenReady.observe(world,{attributes:true,attributeFilter:['class']});
}
