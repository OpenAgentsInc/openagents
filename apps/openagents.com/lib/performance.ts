// Performance monitoring and optimization utilities
import React from 'react'

// Web Vitals tracking
export interface WebVitalsData {
  name: string
  value: number
  delta: number
  id: string
  url: string
  timestamp: number
}

// Performance metrics collection
export class PerformanceMonitor {
  private metrics: Map<string, number[]> = new Map()
  private observers: PerformanceObserver[] = []

  constructor() {
    this.initializeObservers()
  }

  private initializeObservers() {
    if (typeof window === 'undefined') return

    // Observe Core Web Vitals
    if ('PerformanceObserver' in window) {
      try {
        // Largest Contentful Paint (LCP)
        const lcpObserver = new PerformanceObserver((list) => {
          const entries = list.getEntries()
          const lastEntry = entries[entries.length - 1] as any
          if (lastEntry) {
            this.recordMetric('LCP', lastEntry.startTime)
          }
        })
        lcpObserver.observe({ type: 'largest-contentful-paint', buffered: true })
        this.observers.push(lcpObserver)

        // First Input Delay (FID)
        const fidObserver = new PerformanceObserver((list) => {
          const entries = list.getEntries()
          entries.forEach((entry: any) => {
            this.recordMetric('FID', entry.processingStart - entry.startTime)
          })
        })
        fidObserver.observe({ type: 'first-input', buffered: true })
        this.observers.push(fidObserver)

        // Cumulative Layout Shift (CLS)
        let clsValue = 0
        const clsObserver = new PerformanceObserver((list) => {
          const entries = list.getEntries()
          entries.forEach((entry: any) => {
            if (!entry.hadRecentInput) {
              clsValue += entry.value
              this.recordMetric('CLS', clsValue)
            }
          })
        })
        clsObserver.observe({ type: 'layout-shift', buffered: true })
        this.observers.push(clsObserver)

      } catch (error) {
        console.warn('Performance monitoring setup failed:', error)
      }
    }
  }

  recordMetric(name: string, value: number) {
    if (!this.metrics.has(name)) {
      this.metrics.set(name, [])
    }
    this.metrics.get(name)!.push(value)

    // Report to analytics (in production, this would send to your analytics service)
    if (process.env.NODE_ENV === 'production') {
      this.reportMetric(name, value)
    }
  }

  private reportMetric(name: string, value: number) {
    // In production, send to analytics service
    // Example: Google Analytics, DataDog, New Relic, etc.
    console.log(`Performance metric: ${name} = ${value.toFixed(2)}ms`)
  }

  getMetrics(): Record<string, { avg: number, min: number, max: number, count: number }> {
    const result: Record<string, any> = {}
    
    this.metrics.forEach((values, name) => {
      const avg = values.reduce((sum, val) => sum + val, 0) / values.length
      const min = Math.min(...values)
      const max = Math.max(...values)
      
      result[name] = {
        avg: Number(avg.toFixed(2)),
        min: Number(min.toFixed(2)),
        max: Number(max.toFixed(2)),
        count: values.length
      }
    })

    return result
  }

  destroy() {
    this.observers.forEach(observer => observer.disconnect())
    this.observers = []
    this.metrics.clear()
  }
}

// Lazy loading utilities
export function createLazyComponent<T extends React.ComponentType<any>>(
  importFunc: () => Promise<{ default: T }>,
  fallback?: React.ReactNode
) {
  const LazyComponent = React.lazy(importFunc)
  
  return function LazyWrapper(props: React.ComponentProps<T>) {
    return React.createElement(
      React.Suspense,
      { fallback: fallback || React.createElement('div', null, 'Loading...') },
      React.createElement(LazyComponent, props)
    )
  }
}

// Code splitting helper
export function withCodeSplitting<T extends Record<string, any>>(
  moduleLoader: () => Promise<T>
): Promise<T> {
  return moduleLoader()
}

// Resource preloading
export function preloadResource(href: string, as: string = 'script'): void {
  if (typeof window === 'undefined') return

  const link = document.createElement('link')
  link.rel = 'preload'
  link.href = href
  link.as = as
  document.head.appendChild(link)
}

// Bundle size analysis
export function analyzeBundleSize(): void {
  if (typeof window === 'undefined' || process.env.NODE_ENV !== 'development') return

  // In development, log bundle information
  const scripts = Array.from(document.querySelectorAll('script[src]'))
  const stylesheets = Array.from(document.querySelectorAll('link[rel="stylesheet"]'))

  console.group('📦 Bundle Analysis')
  console.log(`Scripts loaded: ${scripts.length}`)
  console.log(`Stylesheets loaded: ${stylesheets.length}`)
  
  if ((navigator as any).connection) {
    const connection = (navigator as any).connection
    console.log(`Connection type: ${connection.effectiveType}`)
    console.log(`Downlink: ${connection.downlink} Mbps`)
  }
  console.groupEnd()
}

// Memory usage monitoring
export function getMemoryUsage(): Record<string, number> | null {
  if (typeof window === 'undefined' || !(performance as any).memory) return null

  const memory = (performance as any).memory
  return {
    usedJSHeapSize: Math.round(memory.usedJSHeapSize / 1024 / 1024), // MB
    totalJSHeapSize: Math.round(memory.totalJSHeapSize / 1024 / 1024), // MB
    jsHeapSizeLimit: Math.round(memory.jsHeapSizeLimit / 1024 / 1024), // MB
  }
}

// Network efficiency
export function getNetworkInfo() {
  if (typeof window === 'undefined' || !(navigator as any).connection) return null

  const connection = (navigator as any).connection
  return {
    effectiveType: connection.effectiveType,
    downlink: connection.downlink,
    rtt: connection.rtt,
    saveData: connection.saveData
  }
}

// Performance recommendations
export function getPerformanceRecommendations(metrics: Record<string, any>): string[] {
  const recommendations: string[] = []

  if (metrics.LCP?.avg > 2500) {
    recommendations.push('🚨 Improve Largest Contentful Paint - consider image optimization and lazy loading')
  }

  if (metrics.FID?.avg > 100) {
    recommendations.push('⚠️ Reduce First Input Delay - optimize JavaScript execution and consider code splitting')
  }

  if (metrics.CLS?.avg > 0.1) {
    recommendations.push('📐 Minimize Cumulative Layout Shift - reserve space for images and ads')
  }

  const memory = getMemoryUsage()
  if (memory && memory.usedJSHeapSize > 50) {
    recommendations.push('🧠 High memory usage detected - consider cleaning up unused objects')
  }

  return recommendations
}

// Global performance monitor instance
export const performanceMonitor = typeof window !== 'undefined' ? new PerformanceMonitor() : null