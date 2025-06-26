import React, { type HTMLProps, type ReactNode } from 'react'
import {
  type AnimatedProp,
  memo,
  Animated,
  FrameCorners,
  Illuminator,
  useBleeps,
  cx
} from '@arwes/react'

interface ButtonSimpleProps extends HTMLProps<HTMLButtonElement> {
  className?: string
  animated?: AnimatedProp
  children: ReactNode
}

const ButtonSimple = memo((props: ButtonSimpleProps): React.ReactElement => {
  const { className, animated, children, ...otherProps } = props

  const bleeps = useBleeps()

  return (
    <Animated<HTMLButtonElement>
      {...otherProps}
      as="button"
      className={cx(
        'relative',
        'group',
        'uppercase font-mono text-[11px] tracking-wider',
        'select-none cursor-pointer transition-all ease-out duration-200',
        'text-yellow-300/80',
        'hover:text-yellow-200',
        'overflow-hidden',
        className
      )}
      animated={animated}
      onMouseEnter={() => {
        bleeps.type?.play()
      }}
      onClick={(event) => {
        otherProps.onClick?.(event)
        bleeps.click?.play()
      }}
    >
      <div className="absolute inset-0 opacity-30 transition-all ease-out duration-200 group-hover:opacity-70">
        <FrameCorners
          className="w-full h-full"
          style={{
            filter: `drop-shadow(0 0 8px hsla(60, 100%, 50%, 0.5))`,
            // @ts-expect-error css variables
            '--arwes-frames-bg-color': 'transparent',
            '--arwes-frames-line-color': 'hsla(60, 100%, 50%, 0.5)',
            '--arwes-frames-deco-color': 'hsla(60, 100%, 50%, 0.6)'
          }}
          animated={false}
          cornerLength={8}
          strokeWidth={1}
          showContentLines={false}
        />
      </div>
      
      <Illuminator
        className="absolute opacity-0 group-hover:opacity-100 transition-opacity duration-300 ease-out pointer-events-none"
        style={{
          inset: 4,
          width: 'calc(100% - 8px)',
          height: 'calc(100% - 8px)'
        }}
        size={120}
        color="hsla(60, 100%, 50%, 0.3)"
      />
      
      <div
        className={cx(
          'relative z-10',
          'flex items-center justify-center gap-1.5',
          'px-4 py-0',
          'h-8',
          'transition-colors duration-200'
        )}
      >
        {children}
      </div>
    </Animated>
  )
})

export { ButtonSimple }