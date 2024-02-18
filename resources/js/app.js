
import { DragGesture } from '@use-gesture/vanilla';
import anime from 'animejs';

document.addEventListener('DOMContentLoaded', () => {
  const el = document.getElementById('drag')
  new DragGesture(el, ({ active, movement: [mx, my] }) => {
    // setActive(active)
    anime({
      targets: el,
      translateX: active ? mx : 0,
      translateY: active ? my : 0,
      duration: active ? 0 : 1000
    })
  })

  // when you want to remove the listener
  // gesture.destroy()
});
