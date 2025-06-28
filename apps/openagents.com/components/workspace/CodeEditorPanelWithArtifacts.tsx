import React from 'react'
import { cx } from '@arwes/react'
import { MonacoEditor } from './MonacoEditor'
import { useCurrentArtifact } from '@/components/artifacts/ArtifactsContext'

interface CodeEditorPanelWithArtifactsProps {
  projectId: string
  className?: string
}

export function CodeEditorPanelWithArtifacts({ projectId, className = '' }: CodeEditorPanelWithArtifactsProps) {
  const { artifact } = useCurrentArtifact()

  if (!artifact) {
    return (
      <div className={cx('h-full bg-offblack flex items-center justify-center', className)}>
        <div className="text-center">
          <div className="text-cyan-500 text-lg mb-2 font-mono">No Code Available</div>
          <div className="text-cyan-300/60 text-sm">Generate code by chatting with AI</div>
        </div>
      </div>
    )
  }

  return (
    <div className={cx('h-full flex', className)}>
      {/* Main Editor */}
      <div className="flex-1 bg-offblack">
        <MonacoEditor
          value={artifact.content}
          language="typescript"
          onChange={(value) => {
            // TODO: Update artifact content when editing is enabled
            console.log('Code changed:', value)
          }}
        />
      </div>
    </div>
  )
}