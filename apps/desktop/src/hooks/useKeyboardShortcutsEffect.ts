import { useEffect } from 'react'
import { Effect, Runtime, Fiber } from 'effect'
import { usePaneStore } from '@/stores/pane'
import { useHotbarStore } from '@/stores/hotbar'
import { addWindowEventListener, withResources } from '@/utils/resources'

interface KeyboardShortcutsProps {
  newProjectPath: string
  createSession: () => void
  toggleHandTracking: () => void
}

/**
 * Keyboard shortcuts hook using Effect for resource management
 * Demonstrates proper cleanup using Effect's Scope API
 */
export const useKeyboardShortcutsEffect = ({
  newProjectPath,
  createSession,
  toggleHandTracking,
}: KeyboardShortcutsProps) => {
  const { 
    activePaneId, 
    removePane, 
    organizePanes, 
    toggleMetadataPane, 
    toggleSettingsPane, 
    toggleStatsPane 
  } = usePaneStore()
  
  const { setPressedSlot } = useHotbarStore()

  useEffect(() => {
    const program = withResources(
      Effect.gen(function* () {
        const handleKeyDown = (event: KeyboardEvent) => {
          const target = event.target as HTMLElement
          if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') {
            return
          }

          if (event.key === 'Escape' && activePaneId) {
            event.preventDefault()
            removePane(activePaneId)
            return
          }

          const modifier = navigator.platform.toUpperCase().indexOf('MAC') >= 0
            ? event.metaKey
            : event.ctrlKey

          if (!modifier) return

          const digit = parseInt(event.key)
          if (isNaN(digit) || digit < 1 || digit > 9) return

          event.preventDefault()
          
          // Set the slot as pressed
          setPressedSlot(digit, true)
          
          // Add a small delay before executing the action for visual feedback
          setTimeout(() => {
            switch (digit) {
              case 1:
                if (newProjectPath) {
                  createSession()
                }
                break
              case 2:
                organizePanes()
                break
              case 3:
                toggleMetadataPane()
                break
              case 4:
                toggleStatsPane()
                break
              case 7:
                toggleSettingsPane()
                break
              case 9:
                toggleHandTracking()
                break
            }
          }, 50)
        }
        
        const handleKeyUp = (event: KeyboardEvent) => {
          const digit = parseInt(event.key)
          if (!isNaN(digit) && digit >= 1 && digit <= 9) {
            // Release the pressed state after a short delay for better visual feedback
            setTimeout(() => {
              setPressedSlot(digit, false)
            }, 100)
          }
          
          // Also handle when modifier key is released
          if (event.key === 'Meta' || event.key === 'Control') {
            // Clear all pressed slots when modifier is released
            for (let i = 1; i <= 9; i++) {
              setPressedSlot(i, false)
            }
          }
        }

        // Add event listeners with automatic cleanup
        yield* addWindowEventListener('keydown', handleKeyDown)
        yield* addWindowEventListener('keyup', handleKeyUp)
        
        // Log that shortcuts are active
        yield* Effect.log("Keyboard shortcuts activated")
      })
    )

    // Run the effect and store the fiber for cleanup
    const fiber = Runtime.runFork(Runtime.defaultRuntime)(program)

    // Cleanup function
    return () => {
      Runtime.runPromise(Runtime.defaultRuntime)(
        Fiber.interrupt(fiber)
      ).catch(console.error)
    }
  }, [
    toggleMetadataPane, 
    toggleSettingsPane, 
    toggleStatsPane, 
    organizePanes, 
    newProjectPath, 
    createSession, 
    toggleHandTracking, 
    activePaneId, 
    removePane,
    setPressedSlot
  ])
}