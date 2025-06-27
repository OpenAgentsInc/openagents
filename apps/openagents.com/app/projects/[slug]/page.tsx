'use client'

import { useParams } from 'next/navigation'
import { useQuery } from 'convex/react'
import { api } from '@/convex/_generated/api'
import { useState, useEffect } from 'react'
import { ProjectWorkspace } from '@/components/mvp/organisms/ProjectWorkspace.stories'
import { DesktopRequired } from '@/components/mvp/templates/DesktopRequired.stories'
import { ChatInterface } from '@/components/mvp/organisms/ChatInterface.stories'
import { FlexibleProjectWorkspace } from '@/components/workspace/FlexibleProjectWorkspace'
import { CodeEditorPanel } from '@/components/workspace/CodeEditorPanel'
import { LazyChat } from '@/components/LazyComponents'
import { AnimatorGeneralProvider, cx } from '@arwes/react'
import { 
  WorkspaceErrorBoundary,
  ChatErrorBoundary,
  CodeEditorErrorBoundary,
  PreviewErrorBoundary 
} from '@/components/workspace/WorkspaceErrorBoundaries'
import { ToastProvider } from '@/components/Toast'

// Check if device is mobile or viewport is too small
function isMobileDevice() {
  if (typeof window === 'undefined') return false
  
  const width = window.innerWidth
  const isMobile = /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i.test(
    navigator.userAgent
  )
  
  return isMobile || width < 1024
}

// Workspace state management hook
function useWorkspaceState(projectId: string) {
  const [state, setState] = useState({
    layout: 'two-column' as 'two-column',
    rightPanelView: 'code' as 'code' | 'preview',
    overallStatus: 'idle' as 'idle' | 'generating' | 'deploying' | 'complete' | 'error',
    panels: {
      left: { id: 'left', title: 'Chat', type: 'chat' as const, isCollapsed: false, isMaximized: false },
      center: { id: 'center', title: 'Code Editor', type: 'none' as const, isCollapsed: false, isMaximized: false },
      right: { id: 'right', title: 'Preview', type: 'none' as const, isCollapsed: false, isMaximized: false }
    }
  })

  // Load persisted layout from localStorage
  useEffect(() => {
    const savedLayout = localStorage.getItem(`workspace-layout-${projectId}`)
    if (savedLayout) {
      try {
        const layout = JSON.parse(savedLayout)
        setState(prev => ({ ...prev, layout }))
      } catch (e) {
        console.error('Failed to load saved layout:', e)
      }
    }
  }, [projectId])

  // Save layout when it changes
  const updateLayout = (layout: typeof state.layout) => {
    setState(prev => ({ ...prev, layout }))
    localStorage.setItem(`workspace-layout-${projectId}`, JSON.stringify(layout))
  }

  const togglePanel = (panelId: string) => {
    setState(prev => ({
      ...prev,
      panels: {
        ...prev.panels,
        [panelId]: {
          ...prev.panels[panelId as keyof typeof prev.panels],
          isCollapsed: !prev.panels[panelId as keyof typeof prev.panels].isCollapsed
        }
      }
    }))
  }

  const maximizePanel = (panelId: string) => {
    setState(prev => ({
      ...prev,
      panels: {
        ...prev.panels,
        [panelId]: {
          ...prev.panels[panelId as keyof typeof prev.panels],
          isMaximized: !prev.panels[panelId as keyof typeof prev.panels].isMaximized
        }
      }
    }))
  }

  const toggleRightPanel = () => {
    setState(prev => ({
      ...prev,
      rightPanelView: prev.rightPanelView === 'code' ? 'preview' : 'code'
    }))
  }

  return { state, setState, updateLayout, togglePanel, maximizePanel, toggleRightPanel }
}

export default function ProjectWorkspacePage() {
  const params = useParams()
  const slug = params.slug as string
  const [showMobileWarning, setShowMobileWarning] = useState(false)

  // Check for mobile on mount and window resize
  useEffect(() => {
    const checkMobile = () => setShowMobileWarning(isMobileDevice())
    checkMobile()
    window.addEventListener('resize', checkMobile)
    return () => window.removeEventListener('resize', checkMobile)
  }, [])

  // Fetch project data (placeholder for now - will connect to Convex later)
  // const project = useQuery(api.projects.getBySlug, { slug })
  
  // For now, use mock data
  const project = {
    _id: 'mock-project-id',
    name: slug.replace(/-/g, ' ').replace(/\b\w/g, c => c.toUpperCase()),
    slug,
    status: 'deployed' as const,
    deploymentUrl: `https://${slug}.openagents.dev`,
    framework: 'react',
    description: 'A project built with OpenAgents Cloud'
  }

  const { state, setState, updateLayout, togglePanel, maximizePanel, toggleRightPanel } = useWorkspaceState(project._id)

  // Show desktop required screen for mobile users
  if (showMobileWarning) {
    return (
      <AnimatorGeneralProvider>
        <DesktopRequired />
      </AnimatorGeneralProvider>
    )
  }

  // Determine overall status based on project status
  const getOverallStatus = () => {
    // Since project.status is always 'deployed' in our mock data,
    // we'll return 'complete'
    return 'complete'
  }

  // Preview component with live iframe
  const PreviewPanel = () => {
    if (project.deploymentUrl) {
      return (
        <div className="h-full bg-black border border-cyan-900/30 flex flex-col">
          <div className="h-12 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
            <span className="text-cyan-500 text-sm font-mono uppercase tracking-wider">Live Preview</span>
            <div className="ml-auto">
              <a 
                href={project.deploymentUrl} 
                target="_blank" 
                rel="noopener noreferrer"
                className="text-cyan-400 hover:text-cyan-300 text-xs font-mono"
              >
                Open in new tab ↗
              </a>
            </div>
          </div>
          <div className="flex-1">
            <iframe
              src={project.deploymentUrl}
              className="w-full h-full border-0"
              title={`Preview of ${project.name}`}
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
            />
          </div>
        </div>
      )
    }

    return (
      <div className="h-full bg-black/50 border border-cyan-900/30 flex items-center justify-center">
        <div className="text-center">
          <div className="text-cyan-500 text-xl mb-2" style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>Preview</div>
          <div className="text-cyan-300/60 text-sm" style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>
            No deployment URL available
          </div>
          <div className="text-cyan-300/40 text-xs mt-4" style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>
            Deploy project to see live preview
          </div>
        </div>
      </div>
    )
  }

  return (
    <AnimatorGeneralProvider>
      <ToastProvider>
        <WorkspaceErrorBoundary>
          <div className="h-screen bg-black flex flex-col">
            {/* Header */}
            <div className="h-16 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
              <h1 className="text-cyan-500 font-mono text-lg">{project.name}</h1>
              
              {/* Toggle buttons */}
              <div className="ml-8 flex items-center">
                <button
                  onClick={() => setState(prev => ({ ...prev, rightPanelView: 'code' }))}
                  className={cx(
                    'px-4 py-1.5 text-sm font-mono uppercase tracking-wider transition-all',
                    state.rightPanelView === 'code' 
                      ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50' 
                      : 'text-cyan-500/60 hover:text-cyan-400 border border-transparent'
                  )}
                >
                  Code
                </button>
                <button
                  onClick={() => setState(prev => ({ ...prev, rightPanelView: 'preview' }))}
                  className={cx(
                    'px-4 py-1.5 text-sm font-mono uppercase tracking-wider transition-all ml-2',
                    state.rightPanelView === 'preview' 
                      ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50' 
                      : 'text-cyan-500/60 hover:text-cyan-400 border border-transparent'
                  )}
                >
                  Preview
                </button>
              </div>
              
              <div className="ml-auto flex items-center gap-4">
                <span className="text-cyan-300/60 text-sm font-sans">Status: {project.status}</span>
                {project.deploymentUrl && (
                  <a 
                    href={project.deploymentUrl} 
                    target="_blank" 
                    rel="noopener noreferrer"
                    className="text-cyan-500 hover:text-cyan-300 text-sm font-mono"
                  >
                    View Deploy →
                  </a>
                )}
              </div>
            </div>
            
            {/* Workspace */}
            <div className="flex-1 overflow-hidden">
              <FlexibleProjectWorkspace
                leftPanel={
                  <ChatErrorBoundary>
                    <LazyChat projectName={project.name} />
                  </ChatErrorBoundary>
                }
                centerPanel={null}
                rightPanel={
                  state.rightPanelView === 'code' 
                    ? (
                      <CodeEditorErrorBoundary>
                        <CodeEditorPanel projectId={project._id} className="h-full" />
                      </CodeEditorErrorBoundary>
                    ) : (
                      <PreviewErrorBoundary>
                        <PreviewPanel />
                      </PreviewErrorBoundary>
                    )
                }
                layout="two-column-right"
                className="h-full"
              />
            </div>
          </div>
        </WorkspaceErrorBoundary>
      </ToastProvider>
    </AnimatorGeneralProvider>
  )
}