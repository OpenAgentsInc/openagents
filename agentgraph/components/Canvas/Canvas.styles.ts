import { styled } from '../../styles'

export const CanvasFrame = styled('div', {
  width: '100%',
  height: '100%',
  backgroundColor: '#000',
  backgroundImage: `radial-gradient(circle, rgba(255, 255, 255, 0.06) 1px, transparent 1px),
                    radial-gradient(circle, rgba(255, 255, 255, 0.06) 1px, transparent 1px)`,
  backgroundSize: '20px 20px',
})
