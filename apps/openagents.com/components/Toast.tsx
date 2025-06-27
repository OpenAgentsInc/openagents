'use client'

import React, { createContext, useContext, useState, useCallback, ReactNode, useEffect } from 'react'
import { Text, cx } from '@arwes/react'
import { 
  CheckCircleIcon, 
  AlertCircleIcon, 
  InfoIcon, 
  XCircleIcon, 
  XIcon,
  RefreshCwIcon 
} from 'lucide-react'

export type ToastType = 'success' | 'error' | 'warning' | 'info'

export interface Toast {
  id: string
  type: ToastType
  title: string
  message?: string
  duration?: number
  action?: {
    label: string
    onClick: () => void
  }
  persistent?: boolean // Don't auto-dismiss
}

interface ToastContextType {
  toasts: Toast[]
  addToast: (toast: Omit<Toast, 'id'>) => string
  removeToast: (id: string) => void
  clearAllToasts: () => void
}

const ToastContext = createContext<ToastContextType | null>(null)

// Generate unique toast ID
function generateToastId(): string {
  return `toast_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
}

// Toast icon mapping
function getToastIcon(type: ToastType) {
  const iconMap = {
    success: CheckCircleIcon,
    error: XCircleIcon,
    warning: AlertCircleIcon,
    info: InfoIcon
  }
  return iconMap[type]
}

// Toast color classes
function getToastColors(type: ToastType) {
  const colorMap = {
    success: {
      bg: 'bg-green-950/30',
      border: 'border-green-500/40',
      icon: 'text-green-400',
      title: 'text-green-300',
      message: 'text-green-300/80'
    },
    error: {
      bg: 'bg-red-950/30',
      border: 'border-red-500/40',
      icon: 'text-red-400',
      title: 'text-red-300',
      message: 'text-red-300/80'
    },
    warning: {
      bg: 'bg-yellow-950/30',
      border: 'border-yellow-500/40',
      icon: 'text-yellow-400',
      title: 'text-yellow-300',
      message: 'text-yellow-300/80'
    },
    info: {
      bg: 'bg-cyan-950/30',
      border: 'border-cyan-500/40',
      icon: 'text-cyan-400',
      title: 'text-cyan-300',
      message: 'text-cyan-300/80'
    }
  }
  return colorMap[type]
}

// Individual toast component
function ToastItem({ toast, onRemove }: { toast: Toast, onRemove: (id: string) => void }) {
  const [isExiting, setIsExiting] = useState(false)
  const Icon = getToastIcon(toast.type)
  const colors = getToastColors(toast.type)

  // Auto-dismiss timer
  useEffect(() => {
    if (!toast.persistent && toast.duration !== 0) {
      const timer = setTimeout(() => {
        setIsExiting(true)
        setTimeout(() => onRemove(toast.id), 300) // Wait for exit animation
      }, toast.duration || 5000)

      return () => clearTimeout(timer)
    }
  }, [toast.id, toast.duration, toast.persistent, onRemove])

  const handleClose = () => {
    setIsExiting(true)
    setTimeout(() => onRemove(toast.id), 300)
  }

  return (
    <div
      className={cx(
        'relative flex items-start gap-3 p-4 rounded border backdrop-blur-sm',
        'transform transition-all duration-300 ease-out',
        'shadow-lg shadow-black/30',
        colors.bg,
        colors.border,
        isExiting 
          ? 'translate-x-full opacity-0 scale-95' 
          : 'translate-x-0 opacity-100 scale-100'
      )}
      style={{ minWidth: '300px', maxWidth: '450px' }}
    >
      {/* Icon */}
      <Icon className={cx('w-5 h-5 flex-shrink-0 mt-0.5', colors.icon)} />
      
      {/* Content */}
      <div className="flex-1 min-w-0">
        <Text className={cx('font-medium font-sans', colors.title)}>
          {toast.title}
        </Text>
        {toast.message && (
          <Text className={cx('text-sm mt-1 font-sans', colors.message)}>
            {toast.message}
          </Text>
        )}
        
        {/* Action button */}
        {toast.action && (
          <button
            onClick={toast.action.onClick}
            className={cx(
              'inline-flex items-center gap-1 mt-3 px-3 py-1 text-sm',
              'border rounded transition-colors font-sans',
              toast.type === 'error' 
                ? 'border-red-500/50 text-red-300 hover:bg-red-500/20'
                : 'border-cyan-500/50 text-cyan-300 hover:bg-cyan-500/20'
            )}
          >
            <RefreshCwIcon className="w-3 h-3" />
            {toast.action.label}
          </button>
        )}
      </div>
      
      {/* Close button */}
      <button
        onClick={handleClose}
        className={cx(
          'p-1 rounded-full hover:bg-white/10 transition-colors',
          'text-gray-400 hover:text-gray-300'
        )}
      >
        <XIcon className="w-4 h-4" />
      </button>
    </div>
  )
}

// Toast container component
function ToastContainer({ toasts, onRemove }: { toasts: Toast[], onRemove: (id: string) => void }) {
  if (toasts.length === 0) return null

  return (
    <div className="fixed top-4 right-4 z-50 space-y-3">
      {toasts.map(toast => (
        <ToastItem key={toast.id} toast={toast} onRemove={onRemove} />
      ))}
    </div>
  )
}

// Toast provider component
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([])

  const addToast = useCallback((toastData: Omit<Toast, 'id'>) => {
    const id = generateToastId()
    const newToast: Toast = {
      id,
      duration: 5000, // Default 5 seconds
      ...toastData
    }

    setToasts(current => {
      // Limit to max 5 toasts to prevent spam
      const updated = [newToast, ...current].slice(0, 5)
      return updated
    })

    return id
  }, [])

  const removeToast = useCallback((id: string) => {
    setToasts(current => current.filter(toast => toast.id !== id))
  }, [])

  const clearAllToasts = useCallback(() => {
    setToasts([])
  }, [])

  return (
    <ToastContext.Provider value={{ toasts, addToast, removeToast, clearAllToasts }}>
      {children}
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </ToastContext.Provider>
  )
}

// Hook to use toast functionality
export function useToast() {
  const context = useContext(ToastContext)
  if (!context) {
    throw new Error('useToast must be used within a ToastProvider')
  }

  const { addToast, removeToast, clearAllToasts } = context

  // Convenience methods for different toast types
  const toast = {
    success: (title: string, message?: string, options?: Partial<Omit<Toast, 'id' | 'type' | 'title' | 'message'>>) =>
      addToast({ type: 'success', title, message, ...options }),
    
    error: (title: string, message?: string, options?: Partial<Omit<Toast, 'id' | 'type' | 'title' | 'message'>>) =>
      addToast({ type: 'error', title, message, ...options }),
    
    warning: (title: string, message?: string, options?: Partial<Omit<Toast, 'id' | 'type' | 'title' | 'message'>>) =>
      addToast({ type: 'warning', title, message, ...options }),
    
    info: (title: string, message?: string, options?: Partial<Omit<Toast, 'id' | 'type' | 'title' | 'message'>>) =>
      addToast({ type: 'info', title, message, ...options }),

    dismiss: removeToast,
    clear: clearAllToasts
  }

  // Promise-based toast for async operations
  const promiseToast = async (
    promise: Promise<any>,
    options: {
      loading: string
      success: string | ((data: any) => string)
      error: string | ((error: Error) => string)
    }
  ) => {
    const { loading, success, error } = options
    const loadingToastId = addToast({
      type: 'info',
      title: loading,
      persistent: true
    })

    try {
      const data = await promise
      removeToast(loadingToastId)
      const successMessage = typeof success === 'function' ? success(data) : success
      addToast({ type: 'success', title: successMessage })
      return data
    } catch (err) {
      removeToast(loadingToastId)
      const errorMessage = typeof error === 'function' && err instanceof Error 
        ? error(err) 
        : typeof error === 'string' 
        ? error 
        : 'An error occurred'
      addToast({ type: 'error', title: errorMessage })
      throw err
    }
  }

  // Add promise method to the toast object
  ;(toast as any).promise = promiseToast

  return toast
}