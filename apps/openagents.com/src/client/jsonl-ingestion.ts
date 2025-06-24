/**
 * JSONL file ingestion for Claude Code conversation data
 */

// TypeScript interfaces for JSONL data structures
interface TextContentPart {
  type: "text"
  text: string
}

interface ThinkingContentPart {
  type: "thinking"
  thinking: string
  signature: string
}

interface ToolUseContentPart {
  type: "tool_use"
  id: string
  name: string
  input: any
}

interface ToolResultContentPart {
  type: "tool_result"
  tool_use_id: string
  content: any
  is_error: boolean
}

interface UserMessage {
  role: "user"
  content: Array<TextContentPart | ToolResultContentPart>
}

interface AssistantMessageUsage {
  input_tokens: number
  cache_creation_input_tokens: number
  cache_read_input_tokens: number
  output_tokens: number
  service_tier: string
}

interface AssistantMessage {
  id: string
  type: "message"
  role: "assistant"
  model: string
  content: Array<TextContentPart | ThinkingContentPart | ToolUseContentPart>
  stop_reason: string | null
  stop_sequence: string | null
  usage: AssistantMessageUsage
}

interface LogEntryBase {
  uuid: string
  timestamp: string
  isSidechain: boolean
  userType: "external"
  cwd: string
  sessionId: string
  version: string
}

interface SummaryEntry {
  type: "summary"
  summary: string
  leafUuid: string
}

interface UserEntry extends LogEntryBase {
  type: "user"
  parentUuid: string | null
  message: UserMessage
  isCompactSummary?: boolean
  toolUseResult?: any
}

interface AssistantEntry extends LogEntryBase {
  type: "assistant"
  parentUuid: string
  message: AssistantMessage
  requestId: string
}

type LogEntry = SummaryEntry | UserEntry | AssistantEntry

export class JSONLIngestion {
  private dropZone: HTMLElement | null = null
  private dragCounter = 0

  /**
   * Initialize drag and drop on the chat interface
   */
  initialize() {
    // Use the entire chat container as the drop zone
    this.dropZone = document.querySelector(".layout-wrapper") || document.body

    if (!this.dropZone) {
      console.error("Could not find drop zone element")
      return
    } // Prevent default drag behaviors
    ;["dragenter", "dragover", "dragleave", "drop"].forEach((eventName) => {
      document.addEventListener(eventName, this.preventDefaults, false)
      this.dropZone!.addEventListener(eventName, this.preventDefaults, false)
    })

    // Add drag and drop event listeners
    this.dropZone.addEventListener("dragenter", this.handleDragEnter.bind(this))
    this.dropZone.addEventListener("dragleave", this.handleDragLeave.bind(this))
    this.dropZone.addEventListener("dragover", this.handleDragOver.bind(this))
    this.dropZone.addEventListener("drop", this.handleDrop.bind(this))

    console.log("JSONL ingestion initialized - drag and drop enabled")
  }

  private preventDefaults(e: Event) {
    e.preventDefault()
    e.stopPropagation()
  }

  private handleDragEnter(_e: DragEvent) {
    this.dragCounter++
    if (this.dragCounter === 1) {
      this.showDropOverlay()
    }
  }

  private handleDragLeave(_e: DragEvent) {
    this.dragCounter--
    if (this.dragCounter === 0) {
      this.hideDropOverlay()
    }
  }

  private handleDragOver(e: DragEvent) {
    // Necessary to allow drop
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = "copy"
    }
  }

  private async handleDrop(e: DragEvent) {
    this.dragCounter = 0
    this.hideDropOverlay()

    if (!e.dataTransfer?.files || e.dataTransfer.files.length === 0) {
      return
    }

    const file = e.dataTransfer.files[0]

    // Check if it's a JSONL file
    if (!file.name.endsWith(".jsonl") && !file.name.endsWith(".json")) {
      console.error("Please drop a JSONL or JSON file")
      this.showError("Please drop a JSONL or JSON file")
      return
    }

    console.log("Processing file:", file.name)

    try {
      const text = await file.text()
      const entries = this.parseJSONL(text)

      console.log("=== JSONL Ingestion Complete ===")
      console.log("File:", file.name)
      console.log("Total entries:", entries.length)
      console.log("Entries by type:", this.getEntriesByType(entries))
      console.log("Full data:", entries)

      // For now, just log the data as requested
      // TODO: Process and display the conversation data in the chat UI

      this.showSuccess(`Successfully loaded ${entries.length} entries from ${file.name}`)
    } catch (error) {
      console.error("Error processing file:", error)
      this.showError(`Error processing file: ${error instanceof Error ? error.message : String(error)}`)
    }
  }

  private parseJSONL(text: string): Array<LogEntry> {
    const lines = text.trim().split("\n")
    const entries: Array<LogEntry> = []

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim()
      if (line) {
        try {
          const entry = JSON.parse(line)
          entries.push(entry)
        } catch (error) {
          console.warn(`Failed to parse line ${i + 1}:`, error instanceof Error ? error.message : String(error))
        }
      }
    }

    return entries
  }

  private getEntriesByType(entries: Array<LogEntry>) {
    const summary = entries.filter((e) => e.type === "summary")
    const user = entries.filter((e) => e.type === "user")
    const assistant = entries.filter((e) => e.type === "assistant")

    return {
      summary: summary.length,
      user: user.length,
      assistant: assistant.length
    }
  }

  private showDropOverlay() {
    // Create overlay if it doesn't exist
    let overlay = document.getElementById("jsonl-drop-overlay")
    if (!overlay) {
      overlay = document.createElement("div")
      overlay.id = "jsonl-drop-overlay"
      overlay.innerHTML = `
        <div class="drop-overlay-content">
          <svg class="drop-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"/>
          </svg>
          <div class="drop-text">Drop your JSONL file here</div>
        </div>
      `
      document.body.appendChild(overlay)
    }
    overlay.classList.add("visible")
  }

  private hideDropOverlay() {
    const overlay = document.getElementById("jsonl-drop-overlay")
    if (overlay) {
      overlay.classList.remove("visible")
    }
  }

  private showError(message: string) {
    this.showNotification(message, "error")
  }

  private showSuccess(message: string) {
    this.showNotification(message, "success")
  }

  private showNotification(message: string, type: "error" | "success") {
    // Create notification element
    const notification = document.createElement("div")
    notification.className = `jsonl-notification ${type}`
    notification.textContent = message

    // Add to page
    document.body.appendChild(notification)

    // Trigger animation
    setTimeout(() => {
      notification.classList.add("visible")
    }, 10)

    // Remove after 3 seconds
    setTimeout(() => {
      notification.classList.remove("visible")
      setTimeout(() => {
        notification.remove()
      }, 300)
    }, 3000)
  }
}

// Export a singleton instance
export const jsonlIngestion = new JSONLIngestion()
