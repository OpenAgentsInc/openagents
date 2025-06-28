'use client'

import React from 'react'
import { cx } from '@arwes/react'
import { WorkspaceChatWithArtifacts } from '@/components/workspace/WorkspaceChatWithArtifacts'
import { ArtifactsPanel } from './ArtifactsPanel'
import { ArtifactsProvider, useArtifactOperations } from './ArtifactsContext'
import { useToast } from '@/components/Toast'

interface ArtifactsWorkspaceProps {
  className?: string
}

function ArtifactsWorkspaceInner({ className = '' }: ArtifactsWorkspaceProps) {
  const toast = useToast()

  return (
    <div className={cx('h-full bg-black flex', className)}>
      {/* Chat Panel - Left Side */}
      <div className="w-1/2 border-r border-cyan-900/30 flex flex-col">
        <div className="flex-1">
          <WorkspaceChatWithArtifacts
            projectName="OpenAgents"
            projectId="workspace"
            className="h-full"
            onArtifactCreated={(artifactId) => {
              toast.success('Code Ready!', 'Your code has been added to the artifacts panel')
            }}
          />
        </div>
      </div>

      {/* Artifacts Panel - Right Side */}
      <div className="w-1/2 flex flex-col">
        <ArtifactsPanel className="h-full" />
      </div>
    </div>
  )
}

// Main component with provider
export function ArtifactsWorkspace(props: ArtifactsWorkspaceProps) {
  return (
    <ArtifactsProvider>
      <ArtifactsWorkspaceInner {...props} />
    </ArtifactsProvider>
  )
}