/**
 * ProseMirror editor setup for chat input
 */

import { baseKeymap } from "prosemirror-commands"
import { keymap } from "prosemirror-keymap"
import { Schema } from "prosemirror-model"
import { EditorState, Plugin } from "prosemirror-state"
import type { Transaction } from "prosemirror-state"
import { EditorView } from "prosemirror-view"

// Define a minimal schema with just doc, paragraph, text, and hard break
const schema = new Schema({
  nodes: {
    doc: {
      content: "paragraph+"
    },
    paragraph: {
      content: "inline*",
      toDOM() {
        return ["p", 0]
      },
      parseDOM: [{ tag: "p" }]
    },
    text: {
      inline: true
    },
    hard_break: {
      inline: true,
      group: "inline",
      selectable: false,
      toDOM() {
        return ["br"]
      },
      parseDOM: [{ tag: "br" }]
    }
  }
})

// Create submit command
function submitCommand(onSubmit: (text: string) => void) {
  return (state: EditorState, dispatch?: (tr: Transaction) => void): boolean => {
    // Get the plain text content
    const text = state.doc.textContent.trim()

    if (!text) return false

    if (dispatch) {
      // Clear the editor
      const tr = state.tr.replaceWith(
        0,
        state.doc.content.size,
        schema.nodes.paragraph.create()
      )
      dispatch(tr)

      // Call the submit handler
      onSubmit(text)
    }

    return true
  }
}

// Create new line command (Shift+Enter)
function newLineCommand(state: EditorState, dispatch?: (tr: Transaction) => void): boolean {
  if (dispatch) {
    // Insert a hard break node
    const br = schema.nodes.hard_break.create()
    const tr = state.tr.replaceSelectionWith(br)
    dispatch(tr)
  }
  return true
}

// Create the keymap
function createKeymap(onSubmit: (text: string) => void) {
  // Put our custom keybindings first so they take precedence
  const customKeys = {
    "Enter": submitCommand(onSubmit),
    "Shift-Enter": newLineCommand
  }

  // Filter out Enter from baseKeymap to prevent conflicts
  const filteredBaseKeymap: { [key: string]: any } = {}
  for (const key in baseKeymap) {
    if (key !== "Enter") {
      filteredBaseKeymap[key] = baseKeymap[key]
    }
  }

  return keymap({
    ...customKeys,
    ...filteredBaseKeymap
  })
}

export interface ProseMirrorEditorOptions {
  mount: HTMLElement
  onSubmit: (text: string) => void
  placeholder?: string
}

export class ProseMirrorEditor {
  private view: EditorView
  private placeholder: string

  constructor(options: ProseMirrorEditorOptions) {
    this.placeholder = options.placeholder || "Message OpenAgents..."

    // Create initial editor state
    const state = EditorState.create({
      schema,
      plugins: [
        createKeymap(options.onSubmit),
        // Placeholder plugin
        new Plugin({
          view: () => ({
            update: (view: EditorView) => {
              const isEmpty = view.state.doc.textContent.length === 0
              if (isEmpty) {
                view.dom.setAttribute("data-placeholder", this.placeholder)
                view.dom.classList.add("empty")
              } else {
                view.dom.removeAttribute("data-placeholder")
                view.dom.classList.remove("empty")
              }
            }
          })
        })
      ]
    })

    // Create editor view
    this.view = new EditorView(options.mount, {
      state,
      attributes: {
        class: "prosemirror-editor chat-input",
        spellcheck: "true"
      }
    })

    // Set initial placeholder state
    this.updatePlaceholder()
  }

  private updatePlaceholder() {
    const isEmpty = this.view.state.doc.textContent.length === 0
    if (isEmpty) {
      this.view.dom.setAttribute("data-placeholder", this.placeholder)
      this.view.dom.classList.add("empty")
    } else {
      this.view.dom.removeAttribute("data-placeholder")
      this.view.dom.classList.remove("empty")
    }
  }

  // Get plain text content
  getText(): string {
    return this.view.state.doc.textContent.trim()
  }

  // Clear the editor
  clear() {
    const tr = this.view.state.tr.replaceWith(
      0,
      this.view.state.doc.content.size,
      schema.nodes.paragraph.create()
    )
    this.view.dispatch(tr)
  }

  // Focus the editor
  focus() {
    this.view.focus()
  }

  // Disable/enable the editor
  setDisabled(disabled: boolean) {
    this.view.setProps({
      editable: () => !disabled
    })
    if (disabled) {
      this.view.dom.classList.add("disabled")
    } else {
      this.view.dom.classList.remove("disabled")
    }
  }

  // Destroy the editor
  destroy() {
    this.view.destroy()
  }

  // Get the view instance (for advanced usage)
  getView(): EditorView {
    return this.view
  }
}
