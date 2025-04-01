import { useEffect, RefObject } from "react"

interface UseFocusInputOptions {
  threadId?: string
}

export function useFocusInput(inputRef: RefObject<HTMLTextAreaElement>, options: UseFocusInputOptions = {}) {
  useEffect(() => {
    // Enhanced focus function with multiple attempts
    const forceInputFocus = () => {
      if (!inputRef.current) return

      // Immediate focus
      inputRef.current.focus()

      // Schedule multiple additional focus attempts with increasing delays
      const delays = [10, 50, 100, 200, 300, 500, 800]

      delays.forEach(delay => {
        setTimeout(() => {
          if (inputRef.current) {
            inputRef.current.focus()

            // Try to ensure it's visible
            try {
              inputRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' })
            } catch (_) {
              // Silent fail
            }
          }
        }, delay)
      })
    }

    // Focus the input when the component initially mounts
    forceInputFocus()

    // Handle new chat events
    const handleNewChatEvent = (event: CustomEvent) => {
      if (!options.threadId) {
        const fromButton = event.detail?.fromButton

        if (fromButton) {
          setTimeout(() => forceInputFocus(), 50)
        } else {
          forceInputFocus()
        }
      }
    }

    // Simple focus event handler
    const handleFocusEvent = () => {
      forceInputFocus()
    }

    // Listen for all events
    window.addEventListener('new-chat', handleNewChatEvent as EventListener)
    window.addEventListener('focus-chat-input', handleFocusEvent)

    // Clean up event listeners
    return () => {
      window.removeEventListener('new-chat', handleNewChatEvent as EventListener)
      window.removeEventListener('focus-chat-input', handleFocusEvent)
    }
  }, [inputRef, options.threadId])

  return {
    forceInputFocus: () => {
      if (!inputRef.current) return
      inputRef.current.focus()
    }
  }
}