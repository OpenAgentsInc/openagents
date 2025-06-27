'use client'

import React, { Component, ReactNode, ErrorInfo } from 'react'
import { Text, cx } from '@arwes/react'
import { AlertTriangleIcon, RefreshCwIcon, BugIcon } from 'lucide-react'

interface ErrorBoundaryState {
  hasError: boolean
  error: Error | null
  errorInfo: ErrorInfo | null
  errorId: string
}

interface ErrorBoundaryProps {
  children: ReactNode
  fallback?: (error: Error, errorId: string, retry: () => void) => ReactNode
  onError?: (error: Error, errorInfo: ErrorInfo, errorId: string) => void
  isolate?: boolean // Whether to isolate errors to this boundary
  name?: string // Name for debugging purposes
}

// Generate unique error ID for tracking
function generateErrorId(): string {
  return `err_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
}

// Log error for monitoring (in production, this would send to error tracking service)
function logError(error: Error, errorInfo: ErrorInfo, errorId: string, boundaryName?: string) {
  const errorReport = {
    id: errorId,
    message: error.message,
    stack: error.stack,
    componentStack: errorInfo.componentStack,
    boundary: boundaryName || 'unknown',
    timestamp: new Date().toISOString(),
    userAgent: typeof window !== 'undefined' ? window.navigator.userAgent : 'unknown',
    url: typeof window !== 'undefined' ? window.location.href : 'unknown'
  }

  // In development, log to console
  if (process.env.NODE_ENV === 'development') {
    console.group(`🚨 Error Boundary: ${boundaryName || 'Generic'}`)
    console.error('Error:', error)
    console.error('Component Stack:', errorInfo.componentStack)
    console.error('Error ID:', errorId)
    console.groupEnd()
  }

  // In production, this would be sent to error monitoring service
  // Example: Sentry, LogRocket, Bugsnag, etc.
  if (process.env.NODE_ENV === 'production') {
    // TODO: Integrate with error monitoring service
    // sendToErrorMonitoring(errorReport)
  }
}

// Default fallback component with Arwes styling
function DefaultErrorFallback({ 
  error, 
  errorId, 
  retry,
  boundaryName 
}: { 
  error: Error
  errorId: string
  retry: () => void
  boundaryName?: string
}) {
  return (
    <div className="min-h-[200px] bg-red-950/20 border border-red-500/30 flex flex-col items-center justify-center p-8 text-center">
      <AlertTriangleIcon className="w-12 h-12 text-red-400 mb-4" />
      
      <Text className="text-lg font-medium text-red-300 mb-2 font-sans">
        Something went wrong
      </Text>
      
      <Text className="text-sm text-red-400/80 mb-6 max-w-md font-sans">
        {boundaryName ? `An error occurred in ${boundaryName}. ` : ''}
        Don't worry - your other work is safe. You can try refreshing this section.
      </Text>
      
      <div className="flex items-center gap-4">
        <button
          onClick={retry}
          className={cx(
            'flex items-center gap-2 px-4 py-2',
            'bg-red-500/20 hover:bg-red-500/30',
            'border border-red-500/50 hover:border-red-400/70',
            'text-red-300 hover:text-red-200',
            'transition-all duration-200',
            'font-sans text-sm'
          )}
        >
          <RefreshCwIcon className="w-4 h-4" />
          Try Again
        </button>
        
        <button
          onClick={() => {
            navigator.clipboard.writeText(`Error ID: ${errorId}\nMessage: ${error.message}`)
          }}
          className={cx(
            'flex items-center gap-2 px-4 py-2',
            'border border-gray-500/30 hover:border-gray-400/50',
            'text-gray-400 hover:text-gray-300',
            'transition-all duration-200',
            'font-sans text-sm'
          )}
        >
          <BugIcon className="w-4 h-4" />
          Copy Error ID
        </button>
      </div>
      
      <Text className="text-xs text-gray-500 mt-4 font-mono">
        Error ID: {errorId}
      </Text>
    </div>
  )
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props)
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null,
      errorId: ''
    }
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return {
      hasError: true,
      error,
      errorId: generateErrorId()
    }
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    const errorId = this.state.errorId || generateErrorId()
    
    this.setState({
      error,
      errorInfo,
      errorId
    })

    // Log error
    logError(error, errorInfo, errorId, this.props.name)

    // Call custom error handler if provided
    if (this.props.onError) {
      this.props.onError(error, errorInfo, errorId)
    }
  }

  retry = () => {
    this.setState({
      hasError: false,
      error: null,
      errorInfo: null,
      errorId: ''
    })
  }

  render() {
    if (this.state.hasError && this.state.error) {
      // Use custom fallback if provided
      if (this.props.fallback) {
        return this.props.fallback(this.state.error, this.state.errorId, this.retry)
      }

      // Use default fallback
      return (
        <DefaultErrorFallback
          error={this.state.error}
          errorId={this.state.errorId}
          retry={this.retry}
          boundaryName={this.props.name}
        />
      )
    }

    return this.props.children
  }
}

// Hook for accessing error boundary context
export function useErrorHandler() {
  return {
    reportError: (error: Error, context?: string) => {
      const errorId = generateErrorId()
      logError(error, { componentStack: context || 'Manual report' }, errorId)
      return errorId
    }
  }
}