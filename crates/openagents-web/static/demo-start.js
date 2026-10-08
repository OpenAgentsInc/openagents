import init, { start_demo } from '/components/assets/coder_components_web.js';
try {
  await init();
  start_demo();
} catch (error) {
  const status = document.getElementById('demo-status');
  if (status) {
    status.hidden = false;
    status.textContent = 'The interactive demo could not start. The original preview remains readable.';
  }
  console.error('Coder demo could not start', error);
}
