import { useState, useEffect } from 'react'

export interface ScreenSizeInfo {
  screenWidth: number
  screenHeight: number
  isDesktop: boolean
  isMobile: boolean
  isTablet: boolean
  deviceType: 'mobile' | 'tablet' | 'desktop'
}

export function useScreenSize(desktopBreakpoint: number = 1024): ScreenSizeInfo {
  // Initialize with actual window size if available (client-side)
  const [screenWidth, setScreenWidth] = useState(() => {
    if (typeof window !== 'undefined') {
      return window.innerWidth
    }
    // Default to desktop size to prevent flicker
    return desktopBreakpoint
  })
  
  const [screenHeight, setScreenHeight] = useState(() => {
    if (typeof window !== 'undefined') {
      return window.innerHeight
    }
    return 768 // Reasonable default
  })

  useEffect(() => {
    const updateScreenSize = () => {
      const width = window.innerWidth
      const height = window.innerHeight
      setScreenWidth(width)
      setScreenHeight(height)
    }

    // Update in case SSR provided defaults
    updateScreenSize()

    // Throttled resize handler for performance
    let timeoutId: ReturnType<typeof setTimeout>
    const throttledResize = () => {
      clearTimeout(timeoutId)
      timeoutId = setTimeout(updateScreenSize, 100)
    }

    window.addEventListener('resize', throttledResize)
    
    return () => {
      window.removeEventListener('resize', throttledResize)
      clearTimeout(timeoutId)
    }
  }, [])

  // Determine device type based on screen width
  const isDesktop = screenWidth >= desktopBreakpoint
  const isTablet = screenWidth >= 768 && screenWidth < desktopBreakpoint
  const isMobile = screenWidth < 768

  let deviceType: 'mobile' | 'tablet' | 'desktop'
  if (isDesktop) deviceType = 'desktop'
  else if (isTablet) deviceType = 'tablet'
  else deviceType = 'mobile'

  return {
    screenWidth,
    screenHeight,
    isDesktop,
    isMobile,
    isTablet,
    deviceType
  }
}