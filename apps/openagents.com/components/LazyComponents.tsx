'use client'

import React, { Suspense } from 'react'
import { Text, cx } from '@arwes/react'
import { Code, Monitor, Upload, FileText, Loader2 } from 'lucide-react'

// Loading fallback components with Arwes styling
const CodeEditorSkeleton = () => (
  <div className="w-full h-full bg-black border border-cyan-900/30 rounded flex flex-col">
    <div className="h-12 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
      <Code className="w-4 h-4 text-cyan-400 mr-2 animate-pulse" />
      <Text className="text-sm text-cyan-300 font-mono">Loading Code Editor...</Text>
    </div>
    <div className="flex-1 flex items-center justify-center">
      <div className="text-center">
        <Loader2 className="w-8 h-8 text-cyan-400 animate-spin mx-auto mb-3" />
        <Text className="text-sm text-gray-400 font-sans">
          Initializing Monaco Editor
        </Text>
        <div className="mt-2 flex items-center justify-center gap-1">
          <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse"></div>
          <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse" style={{ animationDelay: '0.2s' }}></div>
          <div className="w-2 h-2 bg-cyan-400 rounded-full animate-pulse" style={{ animationDelay: '0.4s' }}></div>
        </div>
      </div>
    </div>
  </div>
)

const ChatSkeleton = () => (
  <div className="w-full h-full bg-black border border-cyan-900/30 rounded flex flex-col">
    <div className="h-12 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
      <Monitor className="w-4 h-4 text-cyan-400 mr-2 animate-pulse" />
      <Text className="text-sm text-cyan-300 font-mono">Loading Chat Interface...</Text>
    </div>
    <div className="flex-1 p-4 space-y-3">
      {/* Mock message bubbles */}
      <div className="flex justify-start">
        <div className="bg-gray-800/50 rounded-lg p-3 max-w-[80%] animate-pulse">
          <div className="h-4 bg-gray-700 rounded w-32 mb-2"></div>
          <div className="h-4 bg-gray-700 rounded w-24"></div>
        </div>
      </div>
      <div className="flex justify-end">
        <div className="bg-cyan-600/20 rounded-lg p-3 max-w-[80%] animate-pulse">
          <div className="h-4 bg-cyan-700 rounded w-40"></div>
        </div>
      </div>
      <div className="flex justify-start">
        <div className="bg-gray-800/50 rounded-lg p-3 max-w-[80%] animate-pulse">
          <div className="h-4 bg-gray-700 rounded w-48 mb-2"></div>
          <div className="h-4 bg-gray-700 rounded w-36"></div>
        </div>
      </div>
    </div>
    <div className="p-4 border-t border-cyan-900/30">
      <div className="h-12 bg-gray-800/50 rounded animate-pulse"></div>
    </div>
  </div>
)

const ImportSkeleton = () => (
  <div className="w-full h-full bg-black border border-cyan-900/30 rounded flex flex-col items-center justify-center p-8">
    <Upload className="w-12 h-12 text-cyan-400 mb-4 animate-pulse" />
    <Text className="text-lg text-cyan-300 mb-2 font-sans">Loading File Import...</Text>
    <Text className="text-sm text-gray-400 font-sans text-center">
      Preparing drag & drop interface
    </Text>
    <div className="mt-4 flex items-center gap-2">
      <div className="w-3 h-3 bg-cyan-400 rounded-full animate-bounce"></div>
      <div className="w-3 h-3 bg-cyan-400 rounded-full animate-bounce" style={{ animationDelay: '0.1s' }}></div>
      <div className="w-3 h-3 bg-cyan-400 rounded-full animate-bounce" style={{ animationDelay: '0.2s' }}></div>
    </div>
  </div>
)

const FileTreeSkeleton = () => (
  <div className="w-full h-full bg-black border border-cyan-900/30 rounded flex flex-col">
    <div className="h-12 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
      <FileText className="w-4 h-4 text-cyan-400 mr-2 animate-pulse" />
      <Text className="text-sm text-cyan-300 font-mono">Loading File Tree...</Text>
    </div>
    <div className="flex-1 p-4 space-y-2">
      {/* Mock file tree items */}
      {[...Array(8)].map((_, i) => (
        <div key={i} className="flex items-center gap-2 animate-pulse">
          <div className="w-4 h-4 bg-gray-700 rounded"></div>
          <div className={cx(
            'h-4 bg-gray-700 rounded',
            i % 3 === 0 ? 'w-24' : i % 3 === 1 ? 'w-32' : 'w-20'
          )}></div>
        </div>
      ))}
    </div>
  </div>
)

// Lazy-loaded components with proper error boundaries
const LazyMonacoEditor = React.lazy(() => 
  import('@/components/workspace/MonacoEditor').then(module => ({
    default: module.MonacoEditor
  }))
)

const LazyWorkspaceChat = React.lazy(() => 
  import('@/components/workspace/WorkspaceChat').then(module => ({
    default: module.WorkspaceChat
  }))
)

const LazyProjectImport = React.lazy(() => 
  import('@/components/workspace/ProjectImport').then(module => ({
    default: module.ProjectImport
  }))
)

const LazyFileTree = React.lazy(() => 
  import('@/components/workspace/FileTree').then(module => ({
    default: module.FileTree
  }))
)

// Wrapper components with Suspense and error boundaries
export function LazyCodeEditor(props: any) {
  return (
    <Suspense fallback={<CodeEditorSkeleton />}>
      <LazyMonacoEditor {...props} />
    </Suspense>
  )
}

export function LazyChat(props: any) {
  return (
    <Suspense fallback={<ChatSkeleton />}>
      <LazyWorkspaceChat {...props} />
    </Suspense>
  )
}

export function LazyImport(props: any) {
  return (
    <Suspense fallback={<ImportSkeleton />}>
      <LazyProjectImport {...props} />
    </Suspense>
  )
}

export function LazyTree(props: any) {
  return (
    <Suspense fallback={<FileTreeSkeleton />}>
      <LazyFileTree {...props} />
    </Suspense>
  )
}

// Performance-optimized component loader
export function createOptimizedLoader<T extends React.ComponentType<any>>(
  loader: () => Promise<{ default: T }>,
  fallback: React.ReactNode,
  options: {
    preload?: boolean
    timeout?: number
    retries?: number
  } = {}
) {
  const { preload = false, timeout = 10000, retries = 3 } = options

  // Preload component if requested
  if (preload && typeof window !== 'undefined') {
    // Preload after a short delay to not block initial render
    setTimeout(() => {
      loader().catch(() => {
        // Ignore preload failures
      })
    }, 100)
  }

  const LazyComponent = React.lazy(() => {
    let attempts = 0
    
    const loadWithRetry = async (): Promise<{ default: T }> => {
      try {
        const module = await Promise.race([
          loader(),
          new Promise<never>((_, reject) => 
            setTimeout(() => reject(new Error('Timeout')), timeout)
          )
        ])
        return module
      } catch (error) {
        attempts++
        if (attempts < retries) {
          console.warn(`Component load attempt ${attempts} failed, retrying...`, error)
          await new Promise(resolve => setTimeout(resolve, 1000 * attempts))
          return loadWithRetry()
        }
        throw error
      }
    }

    return loadWithRetry()
  })

  return function OptimizedComponent(props: React.ComponentProps<T>) {
    return (
      <Suspense fallback={fallback}>
        <LazyComponent {...props} />
      </Suspense>
    )
  }
}

// Bundle splitting utilities
export const CodeSplitBoundary: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  return (
    <React.Fragment>
      {children}
    </React.Fragment>
  )
}

// Resource hints for better loading performance
export function addResourceHints() {
  if (typeof window === 'undefined') return
  
  // Disabled: These hardcoded chunk paths cause 404s in Next.js 13+
  // Next.js handles its own chunk loading and optimization
  // Keeping function empty to avoid breaking existing code that calls it
}

// Initialize performance optimizations
export function initializePerformanceOptimizations() {
  if (typeof window === 'undefined') return

  // Resource hints disabled - Next.js handles chunk loading automatically
  // addResourceHints()

  // Service Worker registration for caching
  if ('serviceWorker' in navigator && process.env.NODE_ENV === 'production') {
    window.addEventListener('load', () => {
      navigator.serviceWorker.register('/sw.js')
        .then(registration => {
          console.log('SW registered: ', registration)
        })
        .catch(registrationError => {
          console.log('SW registration failed: ', registrationError)
        })
    })
  }

  // Connection-aware loading
  if ('connection' in navigator) {
    const connection = (navigator as any).connection
    if (connection.effectiveType === 'slow-2g' || connection.effectiveType === '2g') {
      // Disable non-essential features for slow connections
      console.log('Slow connection detected, optimizing for performance')
    }
  }
}