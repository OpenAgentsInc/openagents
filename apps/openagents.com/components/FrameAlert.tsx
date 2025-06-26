import React from 'react'
import { Animated, Animator, FrameLines, Illuminator, memo } from '@arwes/react'

interface FrameAlertProps {
  variant?: 'error' | 'warning' | 'success' | 'info'
  showIlluminator?: boolean
  className?: string
}

const variantColors = {
  error: {
    bg: 'hsla(0, 75%, 50%, 0.01)',
    stripe: 'hsla(0, 75%, 60%, 0.2)',
    frame: 'hsla(0, 75%, 50%, 0.5)',
    line: 'hsla(0, 75%, 60%, 1)',
    illuminator: 'hsla(0, 75%, 50%, 0.1)',
    shadow: 'hsla(0, 75%, 50%, 0.1)'
  },
  warning: {
    bg: 'hsla(30, 75%, 50%, 0.01)',
    stripe: 'hsla(30, 75%, 60%, 0.2)',
    frame: 'hsla(30, 75%, 50%, 0.5)',
    line: 'hsla(30, 75%, 60%, 1)',
    illuminator: 'hsla(30, 75%, 50%, 0.1)',
    shadow: 'hsla(30, 75%, 50%, 0.1)'
  },
  success: {
    bg: 'hsla(120, 75%, 50%, 0.01)',
    stripe: 'hsla(120, 75%, 60%, 0.2)',
    frame: 'hsla(120, 75%, 50%, 0.5)',
    line: 'hsla(120, 75%, 60%, 1)',
    illuminator: 'hsla(120, 75%, 50%, 0.1)',
    shadow: 'hsla(120, 75%, 50%, 0.1)'
  },
  info: {
    bg: 'hsla(180, 75%, 50%, 0.01)',
    stripe: 'hsla(180, 75%, 60%, 0.2)',
    frame: 'hsla(180, 75%, 50%, 0.5)',
    line: 'hsla(180, 75%, 60%, 1)',
    illuminator: 'hsla(180, 75%, 50%, 0.1)',
    shadow: 'hsla(180, 75%, 50%, 0.1)'
  }
}

export const FrameAlert = memo(({ 
  variant = 'error',
  showIlluminator = true,
  className = ''
}: FrameAlertProps): React.ReactElement => {
  const colors = variantColors[variant]
  
  return (
    <Animated
      role="presentation"
      className={`absolute inset-0 ${className}`}
      style={{
        background: `repeating-linear-gradient(-45deg, ${colors.bg}, ${colors.bg} 5px, transparent 5px, transparent 10px)`
      }}
    >
      <div className="absolute inset-0 overflow-hidden">
        {showIlluminator && (
          <Illuminator 
            size={400} 
            color={colors.illuminator}
          />
        )}

        <Animator>
          {/* Top stripe animation */}
          <Animated
            className="absolute -left-20 right-0 top-0 h-[10%]"
            style={{
              background: `repeating-linear-gradient(-45deg, ${colors.stripe}, ${colors.stripe} 12px, transparent 12px, transparent 24px)`
            }}
            animated={['fade', ['x', 80, 0]]}
          />
          {/* Bottom stripe animation */}
          <Animated
            className="absolute left-0 -right-20 bottom-0 h-[10%]"
            style={{
              background: `repeating-linear-gradient(-45deg, ${colors.stripe}, ${colors.stripe} 12px, transparent 12px, transparent 24px)`
            }}
            animated={['fade', ['x', -80, 0]]}
          />
        </Animator>
      </div>

      <FrameLines
        style={{
          // @ts-expect-error css variables
          '--arwes-frames-bg-color': colors.frame,
          '--arwes-frames-line-color': colors.line,
          '--arwes-frames-deco-color': colors.line,
          filter: `drop-shadow(0 0 4px ${colors.shadow})`
        }}
        largeLineWidth={2}
        smallLineWidth={4}
        smallLineLength={24}
      />
    </Animated>
  )
})