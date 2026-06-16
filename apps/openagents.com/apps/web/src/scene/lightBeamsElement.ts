import { makeAnimationView } from './animations/element'
import { mountPylonDiamonds } from './pylonDiamonds'

export const lightBeamsView = makeAnimationView('oa-light-beams', element =>
  mountPylonDiamonds(element, {
    backgroundColor: 0x000000,
    rotationSpeed: 0.00012,
    transparentBackground: true,
  }),
)
