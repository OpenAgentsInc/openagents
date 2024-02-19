import { DragGesture } from '@use-gesture/vanilla';

document.addEventListener('livewire:navigated', () => {
  // Select all elements that need to be draggable
  document.querySelectorAll('.node').forEach(el => {
    let initialX = parseInt(el.style.left, 10);
    let initialY = parseInt(el.style.top, 10);
    const nodeId = el.getAttribute('data-node-id');

    new DragGesture(el, ({ down, movement: [mx, my] }) => {
      if (down) {
        el.style.left = `${initialX + mx}px`;
        el.style.top = `${initialY + my}px`;
      } else {
        initialX += mx;
        initialY += my;
        Livewire.dispatch('updateNodePosition', {
          nodeId, x: initialX, y: initialY
        });
      }
    });
  });

  // Livewire.on('updateNodePosition', (hmm) => {
  //   console.log(hmm)
  // });
});
