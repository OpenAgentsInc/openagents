import { useEffect, RefObject } from "react"

interface UseFocusInputOptions {
  threadId?: string
}

export function useFocusInput(inputRef: RefObject<HTMLTextAreaElement>, options: UseFocusInputOptions = {}) {
  // return null
  useEffect(() => {
    // Enhanced focus function with multiple attempts
    const forceInputFocus = () => {
      if (!inputRef.current) return

      // Immediate focus
      inputRef.current.focus()

      // Just one delayed focus attempt to ensure it works after layout stabilizes
      setTimeout(() => {
        if (inputRef.current) {
          inputRef.current.focus()
        }
      }, 100)
    }

    // Focus the input when the component initially mounts
    // Use a small delay to ensure DOM is settled
    setTimeout(() => forceInputFocus(), 50)

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
