import React, { type ReactElement } from 'react'
import { type AnimatedProp, memo, Animated, cx } from '@arwes/react'

interface ArwesLogoTypeProps {
  className?: string
  animated?: AnimatedProp
  text?: string
}

const ArwesLogoType = memo((props: ArwesLogoTypeProps): ReactElement => {
  const { className, animated, text = 'OpenAgents' } = props
  return (
    <Animated
      as="div"
      className={cx('select-none font-bold tracking-wider', className)}
      style={{
        filter: 'drop-shadow(0 0 8px hsla(180, 100%, 70%, 0.5))',
        fontFamily: 'var(--font-berkeley-mono), monospace'
      }}
      animated={animated}
    >
      {text}
    </Animated>
  )
})

export { ArwesLogoType }
