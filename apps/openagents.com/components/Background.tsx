import React, { type ReactElement } from 'react'
import { type AnimatedProp, memo, Animator, Animated, Dots, Puffs, cx, easing } from '@arwes/react'

interface BackgroundProps {
  className?: string
  animated?: AnimatedProp
}

const Background = memo((props: BackgroundProps): ReactElement => {
  const { className, animated } = props

  return (
    <Animated
      role="presentation"
      className={cx('absolute inset-0 overflow-hidden select-none', className)}
      style={{
        background: 'radial-gradient(50% 50% at 50% 50%, #001515 0%, #000808 50%, #000000 100%)'
      }}
      animated={animated}
    >
      <Animator duration={{ enter: 1 }}>
        <Animated
          as="div"
          className="absolute inset-0"
          style={{
            background: `
              radial-gradient(circle at 50% 0%, hsla(180, 100%, 50%, 0.08) 0%, transparent 50%),
              radial-gradient(circle at 20% 80%, hsla(180, 100%, 50%, 0.04) 0%, transparent 40%),
              radial-gradient(circle at 80% 80%, hsla(180, 100%, 50%, 0.04) 0%, transparent 40%)
            `
          }}
          animated={{
            initialStyle: {
              opacity: 0.6
            },
            transitions: {
              entering: {
                opacity: [0.6, 1],
                easing: easing.outExpo
              }
            }
          }}
        />
      </Animator>

      <Animator duration={{ enter: 1 }}>
        <Dots 
          color="hsla(180, 50%, 50%, 0.15)" 
          size={2} 
          distance={40}
          originInverted 
        />
      </Animator>

      <Animator duration={{ enter: 1, interval: 8 }}>
        <Puffs 
          color="hsla(180, 50%, 50%, 0.25)" 
          quantity={15}
          xOffset={[100, -100]}
          yOffset={[50, -50]}
        />
      </Animator>
    </Animated>
  )
})

export type { BackgroundProps }
export { Background }