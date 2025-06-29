import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook } from '@testing-library/react'
import { useToolBasedArtifacts } from '@/hooks/useToolBasedArtifacts'
import { ArtifactParameters } from '@/lib/tools/createArtifactTool'

// Mock the ArtifactsContext
const mockCreateArtifactFromTool = vi.fn()
const mockUpdateArtifactFromTool = vi.fn()
const mockGetArtifactByIdentifier = vi.fn()

vi.mock('@/components/artifacts/ArtifactsContext', () => ({
  useArtifactOperations: () => ({
    createArtifactFromTool: mockCreateArtifactFromTool,
    updateArtifactFromTool: mockUpdateArtifactFromTool,
    getArtifactByIdentifier: mockGetArtifactByIdentifier
  })
}))

describe('useToolBasedArtifacts', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('processArtifactFromStream', () => {
    it('should create a new artifact when operation is create', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      mockCreateArtifactFromTool.mockReturnValue('artifact-123')
      
      const artifactData: ArtifactParameters = {
        identifier: 'test-component',
        title: 'Test Component',
        type: 'react',
        content: 'export default function Test() { return <div>Test</div> }',
        operation: 'create'
      }
      
      const artifactId = result.current.processArtifactFromStream(
        artifactData,
        'tool-call',
        'message-123'
      )
      
      expect(mockCreateArtifactFromTool).toHaveBeenCalledWith(artifactData, 'message-123')
      expect(artifactId).toBe('artifact-123')
    })

    it('should update existing artifact when operation is update and artifact exists', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      
      // Mock existing artifact
      mockGetArtifactByIdentifier.mockReturnValue({
        id: 'existing-component',
        title: 'Existing Component',
        type: 'react',
        content: 'old content'
      })
      
      const updateData: ArtifactParameters = {
        identifier: 'existing-component',
        title: 'Updated Component',
        type: 'react',
        content: 'export default function Updated() { return <div>Updated</div> }',
        operation: 'update'
      }
      
      const artifactId = result.current.processArtifactFromStream(
        updateData,
        'tool-call',
        'message-456'
      )
      
      expect(mockGetArtifactByIdentifier).toHaveBeenCalledWith('existing-component')
      expect(mockUpdateArtifactFromTool).toHaveBeenCalledWith(updateData, 'message-456')
      expect(artifactId).toBe('existing-component')
    })

    it('should fallback to create when operation is update but artifact does not exist', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      
      // Mock no existing artifact
      mockGetArtifactByIdentifier.mockReturnValue(undefined)
      mockCreateArtifactFromTool.mockReturnValue('new-artifact-123')
      
      const updateData: ArtifactParameters = {
        identifier: 'non-existent-component',
        title: 'Non-existent Component',
        type: 'react',
        content: 'export default function New() { return <div>New</div> }',
        operation: 'update'
      }
      
      const artifactId = result.current.processArtifactFromStream(
        updateData,
        'xml-fallback'
      )
      
      expect(mockGetArtifactByIdentifier).toHaveBeenCalledWith('non-existent-component')
      expect(mockCreateArtifactFromTool).toHaveBeenCalledWith(updateData, undefined)
      expect(artifactId).toBe('new-artifact-123')
    })

    it('should handle errors gracefully and return null', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      
      // Mock error in createArtifactFromTool
      mockCreateArtifactFromTool.mockImplementation(() => {
        throw new Error('Creation failed')
      })
      
      const artifactData: ArtifactParameters = {
        identifier: 'error-component',
        title: 'Error Component',
        type: 'react',
        content: 'some content',
        operation: 'create'
      }
      
      const artifactId = result.current.processArtifactFromStream(
        artifactData,
        'tool-call'
      )
      
      expect(artifactId).toBeNull()
    })
  })

  describe('handleStreamData', () => {
    it('should process valid artifact stream data', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      mockCreateArtifactFromTool.mockReturnValue('stream-artifact-123')
      
      const streamData = {
        type: 'artifact',
        operation: 'tool-call',
        artifact: {
          identifier: 'stream-component',
          title: 'Stream Component',
          type: 'react',
          content: 'export default function Stream() { return <div>Stream</div> }',
          operation: 'create',
          toolCallId: 'tool-call-789'
        },
        timestamp: new Date().toISOString()
      }
      
      const artifactId = result.current.handleStreamData(streamData)
      
      expect(mockCreateArtifactFromTool).toHaveBeenCalledWith(
        expect.objectContaining({
          identifier: 'stream-component',
          title: 'Stream Component',
          type: 'react',
          operation: 'create'
        }),
        'tool-call-789'
      )
      expect(artifactId).toBe('stream-artifact-123')
    })

    it('should handle XML fallback stream data', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      mockCreateArtifactFromTool.mockReturnValue('xml-artifact-456')
      
      const streamData = {
        type: 'artifact',
        operation: 'xml-fallback',
        artifact: {
          identifier: 'xml-component',
          title: 'XML Component',
          type: 'html',
          content: '<div>XML parsed content</div>',
          operation: 'create'
        },
        timestamp: new Date().toISOString()
      }
      
      const artifactId = result.current.handleStreamData(streamData)
      
      expect(mockCreateArtifactFromTool).toHaveBeenCalledWith(
        expect.objectContaining({
          identifier: 'xml-component',
          title: 'XML Component',
          type: 'html',
          operation: 'create'
        }),
        undefined // No toolCallId for XML fallback
      )
      expect(artifactId).toBe('xml-artifact-456')
    })

    it('should return null for non-artifact stream data', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      
      const streamData = {
        type: 'text',
        content: 'This is just regular text data'
      }
      
      const artifactId = result.current.handleStreamData(streamData)
      
      expect(artifactId).toBeNull()
      expect(mockCreateArtifactFromTool).not.toHaveBeenCalled()
    })

    it('should return null for null or undefined data', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      
      expect(result.current.handleStreamData(null)).toBeNull()
      expect(result.current.handleStreamData(undefined)).toBeNull()
      expect(result.current.handleStreamData('')).toBeNull()
      expect(result.current.handleStreamData(123)).toBeNull()
    })

    it('should return null for malformed artifact data', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      
      const malformedData = {
        type: 'artifact',
        operation: 'tool-call'
        // Missing artifact property
      }
      
      const artifactId = result.current.handleStreamData(malformedData)
      
      expect(artifactId).toBeNull()
      expect(mockCreateArtifactFromTool).not.toHaveBeenCalled()
    })

    it('should handle stream data with missing toolCallId', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      mockCreateArtifactFromTool.mockReturnValue('no-tool-call-id')
      
      const streamData = {
        type: 'artifact',
        operation: 'tool-call',
        artifact: {
          identifier: 'no-tool-call',
          title: 'No Tool Call ID',
          type: 'javascript',
          content: 'console.log("no tool call id")',
          operation: 'create'
          // No toolCallId property
        },
        timestamp: new Date().toISOString()
      }
      
      const artifactId = result.current.handleStreamData(streamData)
      
      expect(mockCreateArtifactFromTool).toHaveBeenCalledWith(
        expect.objectContaining({
          identifier: 'no-tool-call',
          operation: 'create'
        }),
        undefined // toolCallId should be undefined
      )
      expect(artifactId).toBe('no-tool-call-id')
    })

    it('should handle update operations from stream data', () => {
      const { result } = renderHook(() => useToolBasedArtifacts())
      
      // Mock existing artifact
      mockGetArtifactByIdentifier.mockReturnValue({
        id: 'update-target',
        title: 'Original Title',
        type: 'react',
        content: 'original content'
      })
      
      const updateStreamData = {
        type: 'artifact',
        operation: 'tool-call',
        artifact: {
          identifier: 'update-target',
          title: 'Updated Title',
          type: 'react',
          content: 'updated content',
          operation: 'update',
          toolCallId: 'update-tool-call'
        },
        timestamp: new Date().toISOString()
      }
      
      const artifactId = result.current.handleStreamData(updateStreamData)
      
      expect(mockGetArtifactByIdentifier).toHaveBeenCalledWith('update-target')
      expect(mockUpdateArtifactFromTool).toHaveBeenCalledWith(
        expect.objectContaining({
          identifier: 'update-target',
          title: 'Updated Title',
          operation: 'update'
        }),
        'update-tool-call'
      )
      expect(artifactId).toBe('update-target')
    })
  })
})