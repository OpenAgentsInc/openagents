/**
 * Date utilities for components
 */

export const dateUtils = {
  /**
   * Format a Unix timestamp to a readable date string
   * @param {number} timestamp - Unix timestamp in seconds
   * @returns {string} Formatted date string
   */
  formatTimestamp(timestamp: number): string {
    const date = new Date(timestamp * 1000)
    const now = new Date()
    const diffMs = now.getTime() - date.getTime()
    const diffSecs = Math.floor(diffMs / 1000)
    const diffMins = Math.floor(diffSecs / 60)
    const diffHours = Math.floor(diffMins / 60)
    const diffDays = Math.floor(diffHours / 24)

    if (diffSecs < 60) {
      return "just now"
    } else if (diffMins < 60) {
      return `${diffMins}m ago`
    } else if (diffHours < 24) {
      return `${diffHours}h ago`
    } else if (diffDays < 7) {
      return `${diffDays}d ago`
    } else {
      return date.toLocaleDateString()
    }
  },

  /**
   * Format a date for display in chat messages
   * @param {number} timestamp - Unix timestamp in seconds
   * @returns {string} Formatted time string
   */
  formatChatTime(timestamp: number): string {
    const date = new Date(timestamp * 1000)
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  },

  /**
   * Format relative time (alias for formatTimestamp)
   * @param {number} timestamp - Unix timestamp in seconds
   * @returns {string} Relative time string
   */
  formatRelativeTime(timestamp: number): string {
    return this.formatTimestamp(timestamp)
  }
}