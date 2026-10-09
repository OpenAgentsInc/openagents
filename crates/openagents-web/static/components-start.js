import init, { start } from '/components/assets/coder_components_web.js';
try {
  await init();
  start();
} catch (error) {
  const status = document.getElementById('catalog-status');
  if (status) {
    status.textContent = 'The interactive preview didn\'t load. You can still read the code.';
    status.className = 'catalog-error';
  }
  console.error('Component catalog could not start', error);
}
