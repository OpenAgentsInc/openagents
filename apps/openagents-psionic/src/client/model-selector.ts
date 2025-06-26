/**
 * Model selector functionality
 * Handles model dropdown, selection, and API key validation
 */

interface Model {
  id: string
  name: string
  provider: string
  description?: string
}

let selectedModel: string
let openrouterApiKey: string
let hasServerKey = false

export async function checkConfig() {
  try {
    const response = await fetch("/api/config")
    const config = await response.json()
    hasServerKey = config.hasOpenRouterKey

    // Update UI based on API key availability
    const hasAnyKey = openrouterApiKey || hasServerKey
    document.querySelectorAll(".openrouter-model").forEach((option) => {
      if (!hasAnyKey) {
        option.classList.add("locked")
      } else {
        option.classList.remove("locked")
        const lockIcon = option.querySelector(".lock-icon")
        if (lockIcon) (lockIcon as HTMLElement).style.display = "none"
      }
    })
  } catch (error) {
    console.error("Failed to check config:", error)
  }
}

export function toggleModelDropdown() {
  const dropdown = document.getElementById("model-selector-dropdown")
  if (!dropdown) return

  dropdown.classList.toggle("open")

  // Close dropdown when clicking outside
  if (dropdown.classList.contains("open")) {
    setTimeout(() => {
      document.addEventListener("click", closeDropdownOnClickOutside)
    }, 0)
  }
}

function closeDropdownOnClickOutside(event: MouseEvent) {
  const container = document.querySelector(".model-selector-container")
  if (!container?.contains(event.target as Node)) {
    const dropdown = document.getElementById("model-selector-dropdown")
    dropdown?.classList.remove("open")
    document.removeEventListener("click", closeDropdownOnClickOutside)
  }
}

export function selectModel(modelId: string) {
  const modelConfig = (window as any).AVAILABLE_MODELS as Array<Model>
  const model = modelConfig.find((m) => m.id === modelId)
  if (!model) return

  // Check if OpenRouter API key is needed
  if (model.provider === "openrouter" && !openrouterApiKey && !hasServerKey) {
    const notice = document.getElementById("api-key-notice")
    if (notice) notice.style.display = "block"
    return
  }

  // Update selection
  selectedModel = modelId
  localStorage.setItem("selectedModel", modelId)

  // Update UI
  const modelName = document.getElementById("selected-model-name")
  if (modelName) modelName.textContent = model.name

  const dropdown = document.getElementById("model-selector-dropdown")
  dropdown?.classList.remove("open")
  document.removeEventListener("click", closeDropdownOnClickOutside)

  // Update selected state
  document.querySelectorAll(".model-option").forEach((option) => {
    option.classList.toggle("selected", option.getAttribute("data-model-id") === modelId)
  })
}

export function getSelectedModel() {
  const modelConfig = (window as any).AVAILABLE_MODELS as Array<Model>
  const model = modelConfig.find((m) => m.id === selectedModel)
  return { id: selectedModel, provider: model?.provider || "cloudflare" }
}

export function initializeModelSelector() {
  // Initialize state
  selectedModel = localStorage.getItem("selectedModel") || (window as any).DEFAULT_MODEL
  openrouterApiKey = localStorage.getItem("openrouterApiKey") || ""

  // Update selected model display on load
  const modelConfig = (window as any).AVAILABLE_MODELS as Array<Model>
  const currentModel = modelConfig.find((m) => m.id === selectedModel)
  if (currentModel) {
    const modelName = document.getElementById("selected-model-name")
    if (modelName) modelName.textContent = currentModel.name
  }

  // Check server configuration
  checkConfig()

  // Mark current selection
  document.querySelectorAll(".model-option").forEach((option) => {
    option.classList.toggle("selected", option.getAttribute("data-model-id") === selectedModel)
  }) // Make functions available globally for onclick handlers
  ;(window as any).toggleModelDropdown = toggleModelDropdown
  ;(window as any).selectModel = selectModel
  ;(window as any).getSelectedModel = getSelectedModel

  // Check for API key updates
  window.addEventListener("storage", (e) => {
    if (e.key === "openrouterApiKey") {
      openrouterApiKey = e.newValue || ""
      // Hide API key notice if key was added
      if (openrouterApiKey) {
        const notice = document.getElementById("api-key-notice")
        if (notice) notice.style.display = "none"
      }
      // Re-check config to update UI
      checkConfig()
    }
  })
}

// Ensure the module is not tree-shaken
export default initializeModelSelector
