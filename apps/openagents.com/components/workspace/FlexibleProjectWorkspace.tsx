import React from 'react'
import { cx } from '@arwes/react'

interface FlexibleProjectWorkspaceProps {
  leftPanel: React.ReactNode
  centerPanel?: React.ReactNode | null
  rightPanel: React.ReactNode
  layout?: 'three-column' | 'two-column-left' | 'two-column-right' | 'single-column'
  className?: string
}

export function FlexibleProjectWorkspace({
  leftPanel,
  centerPanel,
  rightPanel,
  layout = 'three-column',
  className = ''
}: FlexibleProjectWorkspaceProps) {
  const getLayoutClasses = () => {
    switch (layout) {
      case 'two-column-left':
        return {
          container: 'grid-cols-2',
          left: 'block',
          center: 'block',
          right: 'hidden'
        }
      case 'two-column-right':
        return {
          container: 'grid-cols-2',
          left: 'hidden',
          center: 'block',
          right: 'block'
        }
      case 'single-column':
        return {
          container: 'grid-cols-1',
          left: 'hidden',
          center: 'block',
          right: 'hidden'
        }
      default: // three-column
        return {
          container: 'grid-cols-3',
          left: 'block',
          center: 'block',
          right: 'block'
        }
    }
  }
  
  const layoutClasses = getLayoutClasses()
  
  return (
    <div className={cx('h-full grid gap-4', layoutClasses.container, className)}>
      <div className={cx('h-full overflow-hidden', layoutClasses.left)}>
        {leftPanel}
      </div>
      {centerPanel !== null && centerPanel !== undefined && (
        <div className={cx('h-full overflow-hidden', layoutClasses.center)}>
          {centerPanel}
        </div>
      )}
      <div className={cx('h-full overflow-hidden', layoutClasses.right)}>
        {rightPanel}
      </div>
    </div>
  )
}