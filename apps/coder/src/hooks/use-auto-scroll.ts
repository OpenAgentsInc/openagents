import { useEffect, useRef, useState, useCallback } from "react"

// How many pixels from the bottom of the container to enable auto-scroll
const ACTIVATION_THRESHOLD = 50
// Minimum pixels of scroll-up movement required to disable auto-scroll
const MIN_SCROLL_UP_THRESHOLD = 10
// Improved scroll behavior - use 'auto' for better performance than 'smooth'
const SCROLL_BEHAVIOR: ScrollBehavior = "auto"

export function useAutoScroll(dependencies: React.DependencyList) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const previousScrollTop = useRef<number | null>(null)
  const isUserScrolling = useRef<boolean>(false)
  const [shouldAutoScroll, setShouldAutoScroll] = useState(true)
  const lastContentHeight = useRef<number>(0)

  // More reliable scrollToBottom with different strategies based on browser behavior
  const scrollToBottom = useCallback(() => {
    if (!containerRef.current) return
    
    // Ensure we respect user control
    if (isUserScrolling.current) return

    const container = containerRef.current
    const { scrollHeight } = container

    // Force layout calculation to get accurate scrollHeight
    void container.offsetHeight

    // Record current content height to detect genuine changes
    lastContentHeight.current = scrollHeight

    // Strategy 1: scrollTo method
    container.scrollTo({
      top: scrollHeight,
      behavior: SCROLL_BEHAVIOR
    })

    // Strategy 2: Direct property assignment (more reliable in some browsers)
    // This is a backup that works better in some cases
    setTimeout(() => {
      if (container && !isUserScrolling.current) {
        container.scrollTop = container.scrollHeight
      }
    }, 0)
  }, [])

  const handleScroll = useCallback(() => {
    if (!containerRef.current) return
    
    const container = containerRef.current
    const { scrollTop, scrollHeight, clientHeight } = container

    const distanceFromBottom = Math.abs(scrollHeight - scrollTop - clientHeight)
    const isAtBottom = distanceFromBottom < ACTIVATION_THRESHOLD

    const isScrollingUp = previousScrollTop.current !== null && 
                          scrollTop < previousScrollTop.current
    
    const scrollUpDistance = previousScrollTop.current !== null
      ? previousScrollTop.current - scrollTop
      : 0

    // Detect deliberate scroll up action
    if (isScrollingUp && scrollUpDistance > MIN_SCROLL_UP_THRESHOLD) {
      isUserScrolling.current = true
      setShouldAutoScroll(false)
    } 
    // Detect when user has scrolled back to bottom
    else if (isAtBottom) {
      isUserScrolling.current = false
      setShouldAutoScroll(true)
    }

    previousScrollTop.current = scrollTop
  }, [])

  const handleTouchStart = useCallback(() => {
    // When user touches the screen, mark that they're taking control
    // Will re-enable auto-scroll if they return to bottom
    isUserScrolling.current = true
  }, [])

  // Initialize scroll on mount
  useEffect(() => {
    if (!containerRef.current) return
    
    previousScrollTop.current = containerRef.current.scrollTop
    
    // Ensure we scroll to bottom on initial load
    // Use slightly longer timeout to ensure all content is rendered
    setTimeout(scrollToBottom, 100)
    
    // Scroll again after a bit longer in case of slow-loading content
    setTimeout(scrollToBottom, 300)
  }, [scrollToBottom])

  // Auto-scroll when dependencies change
  useEffect(() => {
    if (!shouldAutoScroll) return
    
    if (!containerRef.current) return
    
    const container = containerRef.current
    const currentHeight = container.scrollHeight
    
    // Only scroll if content has actually increased
    // This prevents unnecessary scrolling when content changes but doesn't grow
    if (currentHeight > lastContentHeight.current) {
      // Use multiple timeouts for reliability across different browsers/scenarios
      setTimeout(scrollToBottom, 0)
      setTimeout(scrollToBottom, 50)
    }
    
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, dependencies)

  // Add a mutation observer to handle dynamic content changes
  useEffect(() => {
    if (!containerRef.current) return
    
    const observer = new MutationObserver((mutations) => {
      // Only auto-scroll if we're supposed to
      if (!shouldAutoScroll) return
      
      // Check if any mutations actually added content
      const hasAddedNodes = mutations.some(mutation => 
        mutation.addedNodes.length > 0 || 
        mutation.type === 'characterData'
      )
      
      if (hasAddedNodes) {
        scrollToBottom()
      }
    })
    
    observer.observe(containerRef.current, {
      childList: true,
      subtree: true,
      characterData: true
    })
    
    return () => observer.disconnect()
  }, [scrollToBottom, shouldAutoScroll])

  // Expose the API
  return {
    containerRef,
    scrollToBottom,
    handleScroll,
    shouldAutoScroll,
    handleTouchStart,
  }
}
