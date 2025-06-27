'use client'

import React, { ReactNode } from 'react'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { Text, cx } from '@arwes/react'
import { 
  MessageCircleIcon, 
  CodeIcon, 
  MonitorIcon, 
  FolderIcon,
  RefreshCwIcon,
  AlertCircleIcon 
} from 'lucide-react'

// Chat-specific error fallback
function ChatErrorFallback({ error, errorId, retry }: { error: Error, errorId: string, retry: () => void }) {
  return (
    <div className="h-full bg-red-950/10 border border-red-500/20 flex flex-col items-center justify-center p-6">
      <MessageCircleIcon className="w-10 h-10 text-red-400 mb-3" />
      <Text className="text-lg font-medium text-red-300 mb-2 font-sans">Chat Unavailable</Text>
      <Text className="text-sm text-red-400/80 mb-4 text-center max-w-sm font-sans">
        The AI chat encountered an error. Your project is safe and you can continue working with the code editor.
      </Text>
      <button
        onClick={retry}
        className={cx(
          'flex items-center gap-2 px-4 py-2',
          'bg-red-500/20 hover:bg-red-500/30',
          'border border-red-500/50',
          'text-red-300 hover:text-red-200',
          'transition-all duration-200',
          'font-sans text-sm'
        )}
      >
        <RefreshCwIcon className="w-4 h-4" />
        Restart Chat
      </button>
    </div>
  )
}

// Code editor specific error fallback
function CodeEditorErrorFallback({ error, errorId, retry }: { error: Error, errorId: string, retry: () => void }) {
  return (
    <div className="h-full bg-red-950/10 border border-red-500/20 flex flex-col items-center justify-center p-6">
      <CodeIcon className="w-10 h-10 text-red-400 mb-3" />
      <Text className="text-lg font-medium text-red-300 mb-2 font-sans">Editor Error</Text>
      <Text className="text-sm text-red-400/80 mb-4 text-center max-w-sm font-sans">
        The code editor failed to load. You can still use the chat and preview features.
      </Text>
      <div className="flex gap-3">
        <button
          onClick={retry}
          className={cx(
            'flex items-center gap-2 px-4 py-2',
            'bg-red-500/20 hover:bg-red-500/30',
            'border border-red-500/50',
            'text-red-300 hover:text-red-200',
            'transition-all duration-200',
            'font-sans text-sm'
          )}
        >
          <RefreshCwIcon className="w-4 h-4" />
          Reload Editor
        </button>
        <button
          onClick={() => window.open('/projects', '_blank')}
          className={cx(
            'px-4 py-2',
            'border border-gray-500/30',
            'text-gray-400 hover:text-gray-300',
            'transition-all duration-200',
            'font-sans text-sm'
          )}
        >
          Open New Tab
        </button>
      </div>
    </div>
  )
}

// Preview panel specific error fallback
function PreviewErrorFallback({ error, errorId, retry }: { error: Error, errorId: string, retry: () => void }) {
  return (
    <div className="h-full bg-red-950/10 border border-red-500/20 flex flex-col items-center justify-center p-6">
      <MonitorIcon className="w-10 h-10 text-red-400 mb-3" />
      <Text className="text-lg font-medium text-red-300 mb-2 font-sans">Preview Error</Text>
      <Text className="text-sm text-red-400/80 mb-4 text-center max-w-sm font-sans">
        Unable to load the preview. The deployment might be in progress or unavailable.
      </Text>
      <button
        onClick={retry}
        className={cx(
          'flex items-center gap-2 px-4 py-2',
          'bg-red-500/20 hover:bg-red-500/30',
          'border border-red-500/50',
          'text-red-300 hover:text-red-200',
          'transition-all duration-200',
          'font-sans text-sm'
        )}
      >
        <RefreshCwIcon className="w-4 h-4" />
        Retry Preview
      </button>
    </div>
  )
}

// File tree specific error fallback
function FileTreeErrorFallback({ error, errorId, retry }: { error: Error, errorId: string, retry: () => void }) {
  return (
    <div className="h-full bg-red-950/10 border border-red-500/20 flex flex-col items-center justify-center p-6">
      <FolderIcon className="w-10 h-10 text-red-400 mb-3" />
      <Text className="text-lg font-medium text-red-300 mb-2 font-sans">File Tree Error</Text>
      <Text className="text-sm text-red-400/80 mb-4 text-center max-w-sm font-sans">
        Unable to load project files. You can still work with the code editor.
      </Text>
      <button
        onClick={retry}
        className={cx(
          'flex items-center gap-2 px-4 py-2',
          'bg-red-500/20 hover:bg-red-500/30',
          'border border-red-500/50',
          'text-red-300 hover:text-red-200',
          'transition-all duration-200',
          'font-sans text-sm'
        )}
      >
        <RefreshCwIcon className="w-4 h-4" />
        Reload Files
      </button>
    </div>
  )
}

// Workspace-wide error fallback
function WorkspaceErrorFallback({ error, errorId, retry }: { error: Error, errorId: string, retry: () => void }) {
  return (
    <div className="min-h-screen bg-black flex flex-col items-center justify-center p-8">
      <AlertCircleIcon className="w-16 h-16 text-red-400 mb-6" />
      <Text className="text-2xl font-medium text-red-300 mb-4 font-sans">Workspace Error</Text>
      <Text className="text-red-400/80 mb-8 text-center max-w-md font-sans">
        The workspace encountered a critical error. Don't worry - your project data is safe. 
        You can try reloading or return to your projects.
      </Text>
      <div className="flex gap-4">
        <button
          onClick={retry}
          className={cx(
            'flex items-center gap-2 px-6 py-3',
            'bg-red-500/20 hover:bg-red-500/30',
            'border border-red-500/50',
            'text-red-300 hover:text-red-200',
            'transition-all duration-200',
            'font-sans'
          )}
        >
          <RefreshCwIcon className="w-5 h-5" />
          Reload Workspace
        </button>
        <button
          onClick={() => window.location.href = '/projects'}
          className={cx(
            'px-6 py-3',
            'border border-gray-500/30',
            'text-gray-400 hover:text-gray-300',
            'transition-all duration-200',
            'font-sans'
          )}
        >
          Back to Projects
        </button>
      </div>
    </div>
  )
}

// Specialized error boundary components
export function WorkspaceErrorBoundary({ children }: { children: ReactNode }) {
  return (
    <ErrorBoundary
      name="Workspace"
      fallback={(error, errorId, retry) => <WorkspaceErrorFallback error={error} errorId={errorId} retry={retry} />}
      isolate={true}
    >
      {children}
    </ErrorBoundary>
  )
}

export function ChatErrorBoundary({ children }: { children: ReactNode }) {
  return (
    <ErrorBoundary
      name="Chat"
      fallback={(error, errorId, retry) => <ChatErrorFallback error={error} errorId={errorId} retry={retry} />}
    >
      {children}
    </ErrorBoundary>
  )
}

export function CodeEditorErrorBoundary({ children }: { children: ReactNode }) {
  return (
    <ErrorBoundary
      name="CodeEditor"
      fallback={(error, errorId, retry) => <CodeEditorErrorFallback error={error} errorId={errorId} retry={retry} />}
    >
      {children}
    </ErrorBoundary>
  )
}

export function PreviewErrorBoundary({ children }: { children: ReactNode }) {
  return (
    <ErrorBoundary
      name="Preview"
      fallback={(error, errorId, retry) => <PreviewErrorFallback error={error} errorId={errorId} retry={retry} />}
    >
      {children}
    </ErrorBoundary>
  )
}

export function FileTreeErrorBoundary({ children }: { children: ReactNode }) {
  return (
    <ErrorBoundary
      name="FileTree"
      fallback={(error, errorId, retry) => <FileTreeErrorFallback error={error} errorId={errorId} retry={retry} />}
    >
      {children}
    </ErrorBoundary>
  )
}