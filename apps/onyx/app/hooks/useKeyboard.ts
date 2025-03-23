import { useEffect, useRef, useState } from "react"
import { Keyboard, Platform, TextInput } from "react-native"

// Module-level singleton state
let isKeyboardOpening = false
let isKeyboardOpened = false
let listeners: Set<(isOpened: boolean) => void> = new Set()
let isInitialized = false
let globalInputRef: TextInput | null = null

export function useKeyboard() {
  const [isOpened, setIsOpened] = useState(isKeyboardOpened)
  const localRef = useRef<TextInput>(null)

  useEffect(() => {
    // Add component's state setter to listeners
    listeners.add(setIsOpened)

    if (!isInitialized) {
      // Set up listeners only once
      const showEvent = Platform.OS === "ios" ? "keyboardWillShow" : "keyboardDidShow"
      const hideEvent = Platform.OS === "ios" ? "keyboardWillHide" : "keyboardDidHide"

      const showListener = Keyboard.addListener(showEvent, () => {
        isKeyboardOpening = true
        isKeyboardOpened = true
        listeners.forEach(listener => listener(true))
      })

      const hideListener = Keyboard.addListener(hideEvent, () => {
        isKeyboardOpening = false
        isKeyboardOpened = false
        listeners.forEach(listener => listener(false))
      })

      isInitialized = true

      // Cleanup on app unmount
      return () => {
        showListener.remove()
        hideListener.remove()
        isInitialized = false
        listeners.clear()
        globalInputRef = null
      }
    }

    // Cleanup component's listener and ref
    return () => {
      listeners.delete(setIsOpened)
      if (globalInputRef === localRef.current) {
        globalInputRef = null
      }
    }
  }, [])

  // Update global ref whenever local ref changes
  useEffect(() => {
    const checkRef = () => {
      if (localRef.current) {
        globalInputRef = localRef.current
      }
    }

    // Check immediately
    checkRef()

    // And after a short delay to ensure mounting
    const timer = setTimeout(checkRef, 100)

    return () => clearTimeout(timer)
  }, [])

  const show = () => {
    if (globalInputRef) {
      // Force keyboard to show
      if (Platform.OS === 'ios') {
        globalInputRef.focus()
      } else {
        // On Android, sometimes need to blur then focus
        globalInputRef.blur()
        setTimeout(() => {
          if (globalInputRef) {
            globalInputRef.focus()
          }
        }, 50)
      }
    } else {
      // If ref not available, try again after a short delay
      setTimeout(() => {
        console.log('retrying focus', !!globalInputRef)
        if (globalInputRef) {
          console.log('focusing (retry)')
          if (Platform.OS === 'ios') {
            globalInputRef.focus()
          } else {
            globalInputRef.blur()
            setTimeout(() => {
              if (globalInputRef) {
                globalInputRef.focus()
              }
            }, 50)
          }
        }
      }, 50)
    }
  }

  return {
    isOpening: isKeyboardOpening,
    isOpened,
    dismiss: Keyboard.dismiss,
    show,
    ref: localRef,
  }
}
