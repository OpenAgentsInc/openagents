import { startWebAnalytics, trackPageView } from '@/lib/web-analytics'
import { useRouterState } from '@tanstack/react-router'
import { useEffect } from 'react'

export function WebAnalytics() {
  const pathname = useRouterState({
    select: state => state.location.pathname,
  })

  useEffect(() => {
    const timer = window.setTimeout(() => {
      trackPageView(pathname)
    }, 0)
    return () => window.clearTimeout(timer)
  }, [pathname])

  useEffect(() => {
    let stop: () => void = () => undefined
    const timer = window.setTimeout(() => {
      stop = startWebAnalytics()
    }, 0)
    return () => {
      window.clearTimeout(timer)
      stop()
    }
  }, [])

  return null
}
