import { type AnimatorSettings } from '@arwes/react-animator';

export const animatorSettings: AnimatorSettings = {
  duration: {
    enter: 300,
    exit: 300,
    delay: 100,
    offset: 100,
    stagger: 50
  },
  easing: {
    enter: 'ease-out',
    exit: 'ease-in'
  }
};