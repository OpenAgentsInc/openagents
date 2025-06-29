import React from 'react'
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { ArtifactsProvider, useArtifacts, useCurrentArtifact, useArtifactOperations } from '@/components/artifacts/ArtifactsContext'

// Mock localStorage
const localStorageMock = {
  getItem: vi.fn(),
  setItem: vi.fn(),
  removeItem: vi.fn(),
  clear: vi.fn()
}
Object.defineProperty(window, 'localStorage', { value: localStorageMock })

describe('ArtifactsContext', () => {
  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <ArtifactsProvider>{children}</ArtifactsProvider>
  )

  beforeEach(() => {
    vi.clearAllMocks()
    localStorageMock.getItem.mockReturnValue(null)
  })

  afterEach(() => {
    vi.clearAllTimers()
  })

  describe('ArtifactsProvider', () => {
    it('should provide initial empty state', () => {
      const { result } = renderHook(() => useArtifacts(), { wrapper })

      expect(result.current.state.artifacts).toEqual([])
      expect(result.current.state.currentArtifactId).toBeUndefined()
      expect(result.current.state.isLoading).toBe(false)
      expect(result.current.state.isDeploying).toEqual([])
    })

    it('should load artifacts from localStorage on mount', () => {
      const savedArtifacts = [
        {
          id: 'saved-1',
          title: 'Saved Component',
          type: 'javascript',
          content: 'export default function Saved() {}',
          createdAt: '2025-06-28T10:00:00Z',
          updatedAt: '2025-06-28T10:00:00Z'
        }
      ]
      
      localStorageMock.getItem.mockReturnValue(JSON.stringify(savedArtifacts))

      const { result } = renderHook(() => useArtifacts(), { wrapper })

      expect(result.current.state.artifacts).toHaveLength(1)
      expect(result.current.state.artifacts[0].title).toBe('Saved Component')
      expect(result.current.state.currentArtifactId).toBe('saved-1')
    })

    it('should handle corrupted localStorage data gracefully', () => {
      localStorageMock.getItem.mockReturnValue('invalid json {')
      
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
      
      const { result } = renderHook(() => useArtifacts(), { wrapper })

      expect(result.current.state.artifacts).toEqual([])
      expect(consoleSpy).toHaveBeenCalledWith(
        'Failed to load artifacts from localStorage:',
        expect.any(Error)
      )
      
      consoleSpy.mockRestore()
    })
  })

  describe('addArtifact', () => {
    it('should add a new artifact and set it as current', () => {
      const { result } = renderHook(() => ({
        ops: useArtifactOperations(),
        state: useArtifacts()
      }), { wrapper })

      const artifactData = {
        title: 'Test Component',
        type: 'javascript' as const,
        content: 'export default function Test() {}'
      }

      let artifactId: string = ''
      act(() => {
        artifactId = result.current.ops.addArtifact(artifactData)
      })

      expect(result.current.state.state.artifacts).toHaveLength(1)
      expect(result.current.state.state.artifacts[0].title).toBe('Test Component')
      expect(result.current.state.state.artifacts[0].id).toBe(artifactId!)
      expect(result.current.state.state.currentArtifactId).toBe(artifactId!)
    })

    it('should save to localStorage when adding artifact', async () => {
      const { result } = renderHook(() => useArtifactOperations(), { wrapper })

      act(() => {
        result.current.addArtifact({
          title: 'Test',
          type: 'javascript',
          content: 'test content'
        })
      })

      // Wait for the effect to run
      await waitFor(() => {
        expect(localStorageMock.setItem).toHaveBeenCalledWith(
          'openagents-artifacts',
          expect.stringContaining('"title":"Test"')
        )
      })
    })

    it('should generate unique IDs for artifacts', () => {
      const { result } = renderHook(() => useArtifactOperations(), { wrapper })

      let id1: string = '', id2: string = ''
      act(() => {
        id1 = result.current.addArtifact({ title: 'First', type: 'javascript', content: 'content1' })
        id2 = result.current.addArtifact({ title: 'Second', type: 'javascript', content: 'content2' })
      })

      expect(id1).not.toBe(id2)
      expect(id1).toMatch(/^artifact-\d+-[a-z0-9]+$/)
      expect(id2).toMatch(/^artifact-\d+-[a-z0-9]+$/)
    })
  })

  describe('updateArtifact', () => {
    it('should update existing artifact', async () => {
      const { result } = renderHook(() => ({
        ops: useArtifactOperations(),
        state: useArtifacts()
      }), { wrapper })

      let artifactId: string = ''
      act(() => {
        artifactId = result.current.ops.addArtifact({
          title: 'Original',
          type: 'javascript',
          content: 'original content'
        })
      })

      // Small delay to ensure different timestamps
      await new Promise(resolve => setTimeout(resolve, 1))

      act(() => {
        result.current.ops.updateArtifact(artifactId, {
          title: 'Updated',
          content: 'updated content'
        })
      })

      const updatedArtifact = result.current.state.state.artifacts[0]
      expect(updatedArtifact.title).toBe('Updated')
      expect(updatedArtifact.content).toBe('updated content')
      expect(updatedArtifact.updatedAt.getTime()).toBeGreaterThan(
        updatedArtifact.createdAt.getTime()
      )
    })

    it('should not update non-existent artifact', () => {
      const { result } = renderHook(() => useArtifactOperations(), { wrapper })

      act(() => {
        result.current.updateArtifact('non-existent', { title: 'Updated' })
      })

      const { state } = renderHook(() => useArtifacts(), { wrapper }).result.current
      expect(state.artifacts).toEqual([])
    })
  })

  describe('deleteArtifact', () => {
    it('should delete artifact and update current selection', () => {
      const { result } = renderHook(() => ({
        ops: useArtifactOperations(),
        state: useArtifacts()
      }), { wrapper })

      let id1: string = '', id2: string = '', id3: string = ''
      act(() => {
        id1 = result.current.ops.addArtifact({ title: 'First', type: 'javascript', content: 'c1' })
        id2 = result.current.ops.addArtifact({ title: 'Second', type: 'javascript', content: 'c2' })
        id3 = result.current.ops.addArtifact({ title: 'Third', type: 'javascript', content: 'c3' })
      })

      // Current should be id3 (last added)
      expect(result.current.state.state.currentArtifactId).toBe(id3)

      // Delete current artifact
      act(() => {
        result.current.ops.deleteArtifact(id3)
      })

      expect(result.current.state.state.artifacts).toHaveLength(2)
      expect(result.current.state.state.currentArtifactId).toBe(id1) // Should select first remaining
    })

    it('should clear localStorage when deleting all artifacts', async () => {
      const { result } = renderHook(() => useArtifactOperations(), { wrapper })

      let artifactId: string = ''
      act(() => {
        artifactId = result.current.addArtifact({ title: 'Solo', type: 'javascript', content: 'content' })
      })

      act(() => {
        result.current.deleteArtifact(artifactId)
      })

      await waitFor(() => {
        expect(localStorageMock.removeItem).toHaveBeenCalledWith('openagents-artifacts')
      })
    })
  })

  describe('navigation', () => {
    it('should navigate to next artifact', () => {
      const { result } = renderHook(() => ({
        ops: useArtifactOperations(),
        nav: useCurrentArtifact()
      }), { wrapper })

      let id1: string = '', id2: string = '', id3: string = ''
      act(() => {
        id1 = result.current.ops.addArtifact({ title: 'First', type: 'javascript', content: 'c1' })
        id2 = result.current.ops.addArtifact({ title: 'Second', type: 'javascript', content: 'c2' })
        id3 = result.current.ops.addArtifact({ title: 'Third', type: 'javascript', content: 'c3' })
        result.current.nav.setCurrentArtifact(id1)
      })

      expect(result.current.nav.artifact?.id).toBe(id1)

      act(() => {
        result.current.nav.navigateNext()
      })

      expect(result.current.nav.artifact?.id).toBe(id2)

      act(() => {
        result.current.nav.navigateNext()
      })

      expect(result.current.nav.artifact?.id).toBe(id3)

      // Should not go past the last artifact
      act(() => {
        result.current.nav.navigateNext()
      })

      expect(result.current.nav.artifact?.id).toBe(id3)
    })

    it('should navigate to previous artifact', () => {
      const { result } = renderHook(() => ({
        ops: useArtifactOperations(),
        nav: useCurrentArtifact()
      }), { wrapper })

      let id1: string = '', id2: string = '', id3: string = ''
      act(() => {
        id1 = result.current.ops.addArtifact({ title: 'First', type: 'javascript', content: 'c1' })
        id2 = result.current.ops.addArtifact({ title: 'Second', type: 'javascript', content: 'c2' })
        id3 = result.current.ops.addArtifact({ title: 'Third', type: 'javascript', content: 'c3' })
      })

      expect(result.current.nav.artifact?.id).toBe(id3)

      act(() => {
        result.current.nav.navigatePrevious()
      })

      expect(result.current.nav.artifact?.id).toBe(id2)

      act(() => {
        result.current.nav.navigatePrevious()
      })

      expect(result.current.nav.artifact?.id).toBe(id1)

      // Should not go before the first artifact
      act(() => {
        result.current.nav.navigatePrevious()
      })

      expect(result.current.nav.artifact?.id).toBe(id1)
    })
  })

  describe('deployArtifact', () => {
    it('should deploy artifact and update deployment URL', async () => {
      vi.useFakeTimers()
      
      const { result } = renderHook(() => ({
        ops: useArtifactOperations(),
        state: useArtifacts()
      }), { wrapper })

      let artifactId: string = ''
      act(() => {
        artifactId = result.current.ops.addArtifact({
          title: 'Deploy Test',
          type: 'javascript',
          content: 'export default function DeployTest() {}'
        })
      })

      // Start deployment
      let deployPromise: Promise<void>
      act(() => {
        deployPromise = result.current.ops.deployArtifact(artifactId)
      })

      // Should be in deploying state
      expect(result.current.state.state.isDeploying).toContain(artifactId)

      // Fast-forward time to complete deployment
      await act(async () => {
        vi.advanceTimersByTime(2000)
        await deployPromise
      })

      // Should have deployment URL
      const deployedArtifact = result.current.state.state.artifacts[0]
      expect(deployedArtifact.deploymentUrl).toMatch(/^https:\/\/deploy-test-[a-z0-9]+\.openagents\.dev$/)
      expect(result.current.state.state.isDeploying).not.toContain(artifactId)

      vi.useRealTimers()
    })

    it('should handle deployment failure', async () => {
      const { result } = renderHook(() => useArtifactOperations(), { wrapper })

      // Try to deploy non-existent artifact
      await expect(
        result.current.deployArtifact('non-existent')
      ).resolves.toBeUndefined()
    })

    it('should track deployment state correctly', () => {
      const { result } = renderHook(() => ({
        ops: useArtifactOperations(),
        state: useArtifacts()
      }), { wrapper })

      let artifactId: string = ''
      act(() => {
        artifactId = result.current.ops.addArtifact({
          title: 'Test',
          type: 'javascript',
          content: 'content'
        })
      })

      expect(result.current.state.actions.isDeployingArtifact(artifactId)).toBe(false)

      act(() => {
        result.current.ops.deployArtifact(artifactId)
      })

      expect(result.current.state.actions.isDeployingArtifact(artifactId)).toBe(true)
    })
  })

  describe('clearArtifacts', () => {
    it('should clear all artifacts and reset state', () => {
      const { result } = renderHook(() => ({
        ops: useArtifactOperations(),
        state: useArtifacts()
      }), { wrapper })

      act(() => {
        result.current.ops.addArtifact({ title: '1', type: 'javascript', content: 'c1' })
        result.current.ops.addArtifact({ title: '2', type: 'javascript', content: 'c2' })
        result.current.ops.addArtifact({ title: '3', type: 'javascript', content: 'c3' })
      })

      expect(result.current.state.state.artifacts).toHaveLength(3)

      act(() => {
        result.current.ops.clearArtifacts()
      })

      expect(result.current.state.state.artifacts).toEqual([])
      expect(result.current.state.state.currentArtifactId).toBeUndefined()
      expect(localStorageMock.removeItem).toHaveBeenCalledWith('openagents-artifacts')
    })
  })

  describe('getCurrentArtifact', () => {
    it('should return current artifact', () => {
      const { result } = renderHook(() => ({
        ops: useArtifactOperations(),
        state: useArtifacts()
      }), { wrapper })

      let artifactId: string = ''
      act(() => {
        artifactId = result.current.ops.addArtifact({
          title: 'Current Test',
          type: 'javascript',
          content: 'current content'
        })
      })

      const current = result.current.state.actions.getCurrentArtifact()
      expect(current?.id).toBe(artifactId)
      expect(current?.title).toBe('Current Test')
    })

    it('should return undefined when no current artifact', () => {
      const { result } = renderHook(() => useArtifacts(), { wrapper })
      
      const current = result.current.actions.getCurrentArtifact()
      expect(current).toBeUndefined()
    })
  })
})