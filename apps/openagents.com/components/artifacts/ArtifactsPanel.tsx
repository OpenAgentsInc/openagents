'use client'

import React, { useState } from 'react'
import { cx, Text } from '@arwes/react'
import { Copy, Download, ExternalLink, Code, Monitor, ChevronLeft, ChevronRight } from 'lucide-react'
import { CodeEditorPanelWithArtifacts } from '@/components/workspace/CodeEditorPanelWithArtifacts'
import { useToast } from '@/components/Toast'
import { useArtifacts, useCurrentArtifact, useArtifactOperations } from './ArtifactsContext'

interface ArtifactsPanelProps {
  className?: string
}

export function ArtifactsPanel({ className = '' }: ArtifactsPanelProps) {
  const toast = useToast()
  const [viewMode, setViewMode] = useState<'code' | 'preview'>('code')
  
  const { state } = useArtifacts()
  const { artifact: currentArtifact, navigateNext, navigatePrevious } = useCurrentArtifact()
  const { deployArtifact } = useArtifactOperations()
  
  const { artifacts } = state
  const currentIndex = artifacts.findIndex(a => a.id === currentArtifact?.id)
  
  if (!currentArtifact) {
    return (
      <div className={cx('h-full bg-black/50 border border-cyan-900/30 flex items-center justify-center', className)}>
        <div className="text-center">
          <Code className="w-12 h-12 text-cyan-500/20 mx-auto mb-4" />
          <Text className="text-gray-500 font-sans">No artifacts yet</Text>
          <Text className="text-gray-600 text-sm mt-2 font-sans">
            Chat with AI to generate code and see it here
          </Text>
        </div>
      </div>
    )
  }

  // Navigation handlers
  const handlePrevious = () => {
    navigatePrevious()
  }

  const handleNext = () => {
    navigateNext()
  }

  // Action handlers
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(currentArtifact.content)
      toast.success('Copied!', 'Artifact content copied to clipboard')
    } catch (error) {
      toast.error('Copy Failed', 'Could not copy to clipboard')
    }
  }

  const handleDownload = () => {
    const element = document.createElement('a')
    const file = new Blob([currentArtifact.content], { type: 'text/plain' })
    element.href = URL.createObjectURL(file)
    element.download = `${currentArtifact.title.toLowerCase().replace(/\s+/g, '-')}.tsx`
    document.body.appendChild(element)
    element.click()
    document.body.removeChild(element)
    toast.success('Downloaded!', 'Artifact saved to your downloads')
  }

  const handleDeploy = async () => {
    if (currentArtifact) {
      try {
        await deployArtifact(currentArtifact.id)
        toast.success('Deployed!', `${currentArtifact.title} is now live`)
      } catch (error) {
        toast.error('Deploy Failed', 'Could not deploy artifact')
      }
    }
  }

  const handleOpenExternal = () => {
    if (currentArtifact.deploymentUrl) {
      window.open(currentArtifact.deploymentUrl, '_blank')
    }
  }

  // Preview component for live apps
  const PreviewPanel = () => {
    if (currentArtifact.deploymentUrl) {
      return (
        <div className="h-full bg-black border border-cyan-900/30 flex flex-col">
          <div className="h-12 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
            <span className="text-cyan-500 text-sm font-mono uppercase tracking-wider">Live Preview</span>
            <div className="ml-auto">
              <button
                onClick={handleOpenExternal}
                className="text-cyan-400 hover:text-cyan-300 text-xs font-mono flex items-center gap-1"
              >
                Open in new tab
                <ExternalLink size={12} />
              </button>
            </div>
          </div>
          <div className="flex-1">
            <iframe
              src={currentArtifact.deploymentUrl}
              className="w-full h-full border-0"
              title={`Preview of ${currentArtifact.title}`}
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
            />
          </div>
        </div>
      )
    }

    return (
      <div className="h-full bg-black/50 border border-cyan-900/30 flex items-center justify-center">
        <div className="text-center">
          <Monitor className="w-12 h-12 text-cyan-500/20 mx-auto mb-4" />
          <Text className="text-cyan-500 text-xl mb-2 font-sans">Preview</Text>
          <Text className="text-cyan-300/60 text-sm mb-4 font-sans">
            Deploy this artifact to see live preview
          </Text>
          <button
            onClick={handleDeploy}
            className={cx(
              'px-4 py-2 bg-cyan-500/20 hover:bg-cyan-500/30',
              'border border-cyan-500/50 hover:border-cyan-500/70',
              'text-cyan-300 hover:text-cyan-200',
              'transition-all duration-200 font-sans text-sm rounded'
            )}
          >
            Deploy Now
          </button>
        </div>
      </div>
    )
  }

  return (
    <div className={cx('h-full flex flex-col bg-black', className)}>
      {/* Artifact Header */}
      <div className="h-16 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
        {/* Navigation */}
        <div className="flex items-center gap-2">
          <button
            onClick={handlePrevious}
            disabled={currentIndex === 0}
            className={cx(
              'p-1.5 rounded transition-colors',
              currentIndex === 0
                ? 'text-gray-600 cursor-not-allowed'
                : 'text-cyan-400 hover:text-cyan-300 hover:bg-cyan-500/10'
            )}
            title="Previous artifact"
          >
            <ChevronLeft size={16} />
          </button>
          
          <div className="text-center min-w-[60px]">
            <Text className="text-xs text-cyan-300/60 font-mono">
              {currentIndex + 1} of {artifacts.length}
            </Text>
          </div>
          
          <button
            onClick={handleNext}
            disabled={currentIndex === artifacts.length - 1}
            className={cx(
              'p-1.5 rounded transition-colors',
              currentIndex === artifacts.length - 1
                ? 'text-gray-600 cursor-not-allowed'
                : 'text-cyan-400 hover:text-cyan-300 hover:bg-cyan-500/10'
            )}
            title="Next artifact"
          >
            <ChevronRight size={16} />
          </button>
        </div>

        {/* Artifact Info */}
        <div className="flex-1 mx-6">
          <Text className="text-cyan-300 font-medium font-sans truncate">
            {currentArtifact.title}
          </Text>
          {currentArtifact.description && (
            <Text className="text-cyan-300/60 text-xs font-sans truncate">
              {currentArtifact.description}
            </Text>
          )}
        </div>

        {/* View Mode Toggle */}
        <div className="flex items-center gap-2 mr-4">
          <button
            onClick={() => setViewMode('code')}
            className={cx(
              'px-3 py-1.5 text-xs font-mono uppercase tracking-wider transition-all rounded',
              viewMode === 'code'
                ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50'
                : 'text-cyan-500/60 hover:text-cyan-400 border border-transparent'
            )}
          >
            Code
          </button>
          <button
            onClick={() => setViewMode('preview')}
            className={cx(
              'px-3 py-1.5 text-xs font-mono uppercase tracking-wider transition-all rounded',
              viewMode === 'preview'
                ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50'
                : 'text-cyan-500/60 hover:text-cyan-400 border border-transparent'
            )}
          >
            Preview
          </button>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center gap-2">
          <button
            onClick={handleCopy}
            className="p-2 text-cyan-400 hover:text-cyan-300 hover:bg-cyan-500/10 rounded transition-colors"
            title="Copy code"
          >
            <Copy size={16} />
          </button>
          
          <button
            onClick={handleDownload}
            className="p-2 text-cyan-400 hover:text-cyan-300 hover:bg-cyan-500/10 rounded transition-colors"
            title="Download code"
          >
            <Download size={16} />
          </button>
          
          {currentArtifact.deploymentUrl ? (
            <button
              onClick={handleOpenExternal}
              className="p-2 text-green-400 hover:text-green-300 hover:bg-green-500/10 rounded transition-colors"
              title="Open deployed app"
            >
              <ExternalLink size={16} />
            </button>
          ) : (
            <button
              onClick={handleDeploy}
              className={cx(
                'px-3 py-1.5 bg-cyan-500/20 hover:bg-cyan-500/30',
                'border border-cyan-500/50 hover:border-cyan-500/70',
                'text-cyan-300 hover:text-cyan-200',
                'transition-all duration-200 font-sans text-xs rounded'
              )}
              title="Deploy to Cloudflare Workers"
            >
              Deploy
            </button>
          )}
        </div>
      </div>

      {/* Content Area */}
      <div className="flex-1 overflow-hidden">
        {viewMode === 'code' ? (
          <CodeEditorPanelWithArtifacts 
            projectId={currentArtifact.id} 
            className="h-full"
          />
        ) : (
          <PreviewPanel />
        )}
      </div>
    </div>
  )
}