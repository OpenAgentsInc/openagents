import { DragGesture } from '@use-gesture/vanilla';

document.addEventListener('DOMContentLoaded', () => {
  const el = document.getElementById('drag');
  // Initialize initialX and initialY with the element's current position
  let initialX = el.offsetLeft;
  let initialY = el.offsetTop;

  new DragGesture(el, ({ down, movement: [mx, my] }) => {
    if (down) {
      // Update position based on initial position plus movement
      el.style.left = `${initialX + mx}px`;
      el.style.top = `${initialY + my}px`;
    } else {
      // Capture the final position at the end of the drag
      initialX += mx;
      initialY += my;
    }
  });
});
