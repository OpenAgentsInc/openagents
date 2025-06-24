/**
 * Main client-side entry point
 * This file initializes client-side functionality and sets up global utilities
 */

// Import styles
import "./main.css"

// Initialize any global client functionality
console.log("OpenAgents client initialized")

// Export any utilities that need to be globally available
export function initializeClient() {
  // Set up any global event listeners or initialization logic

  // Example: Theme initialization
  const theme = localStorage.getItem("theme") || "zinc"
  document.documentElement.setAttribute("data-theme", theme)

  // Example: Global error handling
  window.addEventListener("error", (event) => {
    console.error("Global error:", event.error)
  })

  // Example: Service worker registration (if needed)
  if ("serviceWorker" in navigator && process.env.NODE_ENV === "production") {
    navigator.serviceWorker.register("/sw.js").catch((error) => {
      console.error("Service worker registration failed:", error)
    })
  }
}

// Auto-initialize on DOM ready
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initializeClient)
} else {
  initializeClient()
}
