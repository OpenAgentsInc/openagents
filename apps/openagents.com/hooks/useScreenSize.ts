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
  const [screenWidth, setScreenWidth] = useState(0)
  const [screenHeight, setScreenHeight] = useState(0)

  useEffect(() => {
    const updateScreenSize = () => {
      const width = window.innerWidth
      const height = window.innerHeight
      setScreenWidth(width)
      setScreenHeight(height)
    }

    // Set initial size
    updateScreenSize()

    // Throttled resize handler for performance
    let timeoutId: NodeJS.Timeout
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