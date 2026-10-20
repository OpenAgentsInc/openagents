// The CAD viewer keeps picking and drawing; the GPUI forms own page controls.
const viewer = document.querySelector('.oa-cad-viewer');
const tell = value => window.parent.postMessage(value, window.location.origin);
if (viewer) {
  viewer.addEventListener('cad-face-pick', event => tell({cadFace: event.detail}));
}
const page = document.querySelector('[data-cad-doc-page]');
if (page) {
  const buttons = [...page.querySelectorAll('[data-cad-download]')];
  const status = page.querySelector('[data-cad-download-status]');
  let ready = false;
  const observer = new MutationObserver(() => {
    if (!ready && buttons.length && buttons.every(b => !b.disabled)) {
      ready = true;
      tell({cadReady: true});
    }
    if (status?.textContent) tell({cadStatus: status.textContent});
  });
  observer.observe(page, {subtree: true, attributes: true, childList: true, characterData: true});
  window.addEventListener('message', event => {
    if (event.origin !== window.location.origin || event.source !== window.parent) return;
    const button = buttons.find(b => b.dataset.cadDownload === event.data);
    if (button && !button.disabled) button.click();
  });
  window.addEventListener('pagehide', () => observer.disconnect());
}
