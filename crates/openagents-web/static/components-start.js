import init, { start } from '/components/assets/coder_components_web.js';
try {
  await init();
  start();
} catch (error) {
  const status = document.getElementById('catalog-status');
  if (status) {
    status.textContent = 'Interactive module unavailable. Source previews remain readable.';
    status.className = 'catalog-error';
  }
  console.error('Component catalog could not start', error);
}
