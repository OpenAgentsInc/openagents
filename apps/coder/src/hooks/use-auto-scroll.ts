import { useEffect, useRef, useState } from "react"

// How many pixels from the bottom of the container to enable auto-scroll
const ACTIVATION_THRESHOLD = 50
// Minimum pixels of scroll-up movement required to disable auto-scroll
const MIN_SCROLL_UP_THRESHOLD = 10
// Smooth scroll behavior for better user experience
const SCROLL_BEHAVIOR: ScrollBehavior = "smooth"

export function useAutoScroll(dependencies: React.DependencyList) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const previousScrollTop = useRef<number | null>(null)
  const isUserScrolling = useRef<boolean>(false)
  const [shouldAutoScroll, setShouldAutoScroll] = useState(true)

  const scrollToBottom = () => {
    if (containerRef.current && !isUserScrolling.current) {
      containerRef.current.scrollTo({
        top: containerRef.current.scrollHeight,
        behavior: SCROLL_BEHAVIOR
      })
    }
  }

  const handleScroll = () => {
    if (containerRef.current) {
      const { scrollTop, scrollHeight, clientHeight } = containerRef.current

      const distanceFromBottom = Math.abs(
        scrollHeight - scrollTop - clientHeight
      )

      const isScrollingUp = previousScrollTop.current
        ? scrollTop < previousScrollTop.current
        : false

      const scrollUpDistance = previousScrollTop.current
        ? previousScrollTop.current - scrollTop
        : 0

      const isDeliberateScrollUp =
        isScrollingUp && scrollUpDistance > MIN_SCROLL_UP_THRESHOLD
      
      // If user is deliberately scrolling up, disable auto-scroll
      if (isDeliberateScrollUp) {
        isUserScrolling.current = true
        setShouldAutoScroll(false)
      } else {
        // If user has scrolled close to bottom, re-enable auto-scroll
        const isScrolledToBottom = distanceFromBottom < ACTIVATION_THRESHOLD
        if (isScrolledToBottom) {
          isUserScrolling.current = false
          setShouldAutoScroll(true)
        }
      }

      previousScrollTop.current = scrollTop
    }
  }

  const handleTouchStart = () => {
    // When user touches the screen, temporarily disable auto-scroll
    // It will re-enable if they scroll back to bottom
    isUserScrolling.current = true
  }

  // Initialize scroll position tracking
  useEffect(() => {
    if (containerRef.current) {
      previousScrollTop.current = containerRef.current.scrollTop
      
      // Initial scroll to bottom
      setTimeout(() => {
        scrollToBottom()
      }, 100)
    }
  }, [])

  // Auto-scroll when dependencies change (messages, parts, typing)
  useEffect(() => {
    if (shouldAutoScroll) {
      // Small delay to ensure DOM updates have completed
      setTimeout(() => {
        scrollToBottom()
      }, 10)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, dependencies)

  return {
    containerRef,
    scrollToBottom,
    handleScroll,
    shouldAutoScroll,
    handleTouchStart,
  }
}
