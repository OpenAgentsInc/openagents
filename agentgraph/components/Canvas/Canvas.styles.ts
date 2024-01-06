import { styled } from '../../styles'

export const CanvasFrame = styled('div', {
  width: '100%',
  height: '100%',
  backgroundColor: '$highlight3',
  backgroundImage: `radial-gradient(circle, rgba(0, 0, 0, 0.26) 1px, transparent 1px),
                    radial-gradient(circle, rgba(0, 0, 0, 0.26) 1px, transparent 1px)`,
  backgroundSize: '20px 20px',
})
