import React from 'react'
import { DesktopRequired } from '@/components/mvp/templates/DesktopRequired.stories'

export interface DesktopRequiredOverlayProps {
  screenWidth: number
  minWidth?: number
  customMessage?: string
  animated?: boolean
  className?: string
}

export const DesktopRequiredOverlay = ({
  screenWidth,
  minWidth = 1024,
  customMessage,
  animated = true,
  className = ''
}: DesktopRequiredOverlayProps): React.ReactElement => {
  const defaultMessage = `OpenAgents requires a desktop computer with a screen width of at least ${minWidth}px for the full development experience. Please use a larger screen to access the platform.`

  return (
    <div className={`fixed inset-0 z-50 bg-black ${className}`}>
      <DesktopRequired
        minWidth={minWidth}
        customMessage={customMessage || defaultMessage}
        animated={animated}
        // No onContinueAnyway - strict blocking per MVP spec
      />
      
      {/* Additional context overlay */}
      <div className="absolute bottom-8 left-1/2 transform -translate-x-1/2 text-center">
        <div className="bg-yellow-900/20 border border-yellow-500/30 rounded-lg p-4 max-w-md">
          <p className="text-yellow-300 text-sm mb-2">
            Current screen: <span className="font-mono font-bold">{screenWidth}px</span> width
          </p>
          <p className="text-yellow-400/80 text-xs">
            OpenAgents is optimized for desktop development workflows with multiple panels, 
            code editing, and real-time previews.
          </p>
        </div>
      </div>
    </div>
  )
}