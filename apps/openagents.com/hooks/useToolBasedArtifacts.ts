'use client'

import { useEffect } from 'react'
import { useArtifactOperations } from '@/components/artifacts/ArtifactsContext'
import { ArtifactParameters } from '@/lib/tools/createArtifactTool'

/**
 * Hook for handling tool-based artifact creation from streaming chat responses
 */
export function useToolBasedArtifacts() {
  const { createArtifactFromTool, updateArtifactFromTool, getArtifactByIdentifier } = useArtifactOperations()

  /**
   * Processes artifact data from the chat stream
   * Handles both tool calls and XML fallback artifacts
   */
  const processArtifactFromStream = (
    artifactData: ArtifactParameters, 
    operation: 'tool-call' | 'xml-fallback',
    messageId?: string
  ): string | null => {
    try {
      // Check if this is an update to an existing artifact
      if (artifactData.operation === 'update') {
        const existingArtifact = getArtifactByIdentifier(artifactData.identifier)
        if (existingArtifact) {
          updateArtifactFromTool(artifactData, messageId)
          return artifactData.identifier
        } else {
          console.warn(`Attempted to update non-existent artifact: ${artifactData.identifier}`)
          // Fallback to creation if artifact doesn't exist
        }
      }

      // Create new artifact
      const artifactId = createArtifactFromTool(artifactData, messageId)
      return artifactId
    } catch (error) {
      console.error('Failed to process artifact from stream:', error)
      return null
    }
  }

  /**
   * Handles data from the chat stream, specifically looking for artifact events
   */
  const handleStreamData = (data: any): string | null => {
    if (!data || typeof data !== 'object') {
      return null
    }

    // Check for artifact data
    if (data.type === 'artifact' && data.artifact) {
      const artifactData = data.artifact as ArtifactParameters & { toolCallId?: string }
      const operation = data.operation as 'tool-call' | 'xml-fallback'
      const messageId = artifactData.toolCallId || undefined

      return processArtifactFromStream(artifactData, operation, messageId)
    }

    return null
  }

  return {
    processArtifactFromStream,
    handleStreamData
  }
}

/**
 * Type for stream data events that contain artifacts
 */
export interface ArtifactStreamEvent {
  type: 'artifact'
  operation: 'tool-call' | 'xml-fallback'
  artifact: ArtifactParameters & { toolCallId?: string }
  timestamp: string
}