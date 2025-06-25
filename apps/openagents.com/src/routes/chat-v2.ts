import { HttpServerResponse } from "@effect/platform"
import { Effect } from "effect"
import { html } from "../lib/html-builder"
import { AVAILABLE_MODELS, DEFAULT_MODEL } from "../lib/models-config"

/**
 * Refactored chat route using HTML builder to avoid template literal issues
 * 
 * This implementation:
 * 1. Builds HTML using string concatenation
 * 2. Properly escapes all user content
 * 3. Returns HttpServerResponse directly
 * 4. Uses Effect.gen for async operations
 */
export function chatV2(ctx: { params: { id: string } }) {
  return Effect.gen(function* () {
    const conversationId = ctx.params.id
    
    // Import server-side dependencies
    const chatClient = yield* Effect.tryPromise({
      try: () => import("../lib/chat-client-convex"),
      catch: () => new Error("Failed to load chat client")
    })
    const { getConversationWithMessages, getConversations } = chatClient
    
    // Load all conversations for sidebar
    let allConversations: Array<any> = []
    try {
      const conversations = yield* Effect.tryPromise({
        try: () => getConversations(),
        catch: () => new Error("Failed to load conversations")
      })
      allConversations = conversations as Array<any>
    } catch (error) {
      console.error("Failed to load conversations:", error)
    }
    
    // Load specific conversation
    let conversation: any = null
    let messages: Array<any> = []
    
    if (conversationId) {
      try {
        const result = yield* Effect.tryPromise({
          try: () => getConversationWithMessages(conversationId),
          catch: () => new Error("Failed to load conversation")
        })
        conversation = result.conversation
        messages = result.messages
      } catch (error) {
        console.error("Failed to load conversation:", error)
      }
    }
    
    // Build HTML using the builder
    const doc = html()
    
    doc.add('<!DOCTYPE html>')
    doc.element('html', { lang: 'en' }, () => {
      doc.element('head', {}, () => {
        doc.element('meta', { charset: 'UTF-8' })
        doc.element('meta', { name: 'viewport', content: 'width=device-width, initial-scale=1.0' })
        doc.element('title', {}, conversation?.title || 'Chat - OpenAgents')
        
        // Add stylesheets
        doc.element('link', { rel: 'stylesheet', href: '/css/client.css' })
        
        // Add inline styles for chat view
        doc.element('style', {}, () => {
          doc.add(`
            /* Chat-specific styles */
            .message {
              display: block;
              margin-bottom: 1.5rem;
              width: 100%;
              max-width: 800px;
              margin-left: auto;
              margin-right: auto;
            }
            
            .message-block {
              border-left: 4px solid var(--color-terminal-accent);
              padding-left: 1rem;
              padding-top: 0.75rem;
              padding-bottom: 0.75rem;
              background: transparent;
              border-radius: 0;
            }
            
            .message-block.user {
              border-left-color: #9ece6a;
            }
            
            .message-block.assistant {
              border-left-color: #7aa2f7;
            }
            
            .message-header {
              display: flex;
              align-items: center;
              gap: 0.5rem;
              margin-bottom: 0.5rem;
            }
            
            .message-role {
              font-size: 0.75rem;
              font-weight: 500;
              text-transform: uppercase;
              opacity: 0.7;
            }
            
            .message-time {
              font-size: 0.75rem;
              opacity: 0.5;
            }
            
            .message-body {
              color: var(--text);
              line-height: 1.6;
            }
            
            .message-body pre {
              white-space: pre-wrap;
              word-wrap: break-word;
              font-family: var(--font-mono);
            }
          `)
        })
      })
      
      doc.element('body', {}, () => {
        doc.element('div', { class: 'app-container' }, () => {
          // Sidebar
          doc.element('aside', { class: 'sidebar' }, () => {
            // Sidebar header
            doc.element('div', { class: 'sidebar-header' }, () => {
              doc.element('button', { class: 'new-chat-button', onclick: 'createNewChat()' }, 'New Chat')
            })
            
            // Thread list
            doc.element('div', { class: 'thread-list' }, () => {
              if (allConversations.length > 0) {
                doc.element('div', { class: 'mt-2' }, () => {
                  doc.element('div', { class: 'px-3 py-1 mb-0.5' }, () => {
                    doc.element('span', { class: 'text-xs font-medium text-[rgba(255,255,255,0.5)] uppercase' }, 'Recent')
                  })
                  
                  doc.element('ul', { class: 'flex flex-col gap-0.5' }, () => {
                    for (const conv of allConversations) {
                      const isActive = conv.id === conversationId
                      const className = isActive
                        ? 'block px-3 py-1.5 text-sm rounded-md transition-colors bg-[rgba(255,255,255,0.1)] text-[#D7D8E5]'
                        : 'block px-3 py-1.5 text-sm rounded-md transition-colors text-[rgba(255,255,255,0.7)] hover:bg-[rgba(255,255,255,0.05)] hover:text-[#D7D8E5]'
                      
                      doc.element('li', {}, () => {
                        doc.element('a', { href: `/chat/${conv.id}`, class: className }, () => {
                          doc.element('span', {}, conv.title || 'Untitled Chat')
                        })
                      })
                    }
                  })
                })
              }
            })
          })
          
          // Main content
          doc.element('main', { class: 'main-content' }, () => {
            // Chat header
            doc.element('header', { class: 'chat-header' }, () => {
              doc.element('h2', { class: 'chat-title' }, conversation?.title || 'New Chat')
            })
            
            // Messages container
            doc.element('div', { id: 'messages-container', class: 'messages-container' }, () => {
              doc.element('div', {}, () => {
                // Render messages
                for (const message of messages) {
                  renderMessage(doc, message)
                }
              })
            })
            
            // Input area
            doc.element('div', { class: 'input-area' }, () => {
              doc.element('div', { class: 'input-container' }, () => {
                doc.element('textarea', {
                  id: 'message-input',
                  class: 'message-input',
                  placeholder: 'Type your message...',
                  rows: '1'
                })
                
                doc.element('button', {
                  id: 'send-button',
                  class: 'send-button',
                  onclick: 'sendMessage()'
                }, 'Send')
              })
            })
          })
        })
        
        // Add scripts
        doc.element('script', {}, () => {
          doc.add(`
            // Set conversation ID globally
            window.CONVERSATION_ID = ${conversationId ? JSON.stringify(conversationId) : 'null'};
            
            // Set model config globally
            window.AVAILABLE_MODELS = ${JSON.stringify(AVAILABLE_MODELS)};
            window.DEFAULT_MODEL = ${JSON.stringify(DEFAULT_MODEL)};
          `)
        })
        
        doc.element('script', { type: 'module', src: '/js/chat.js' })
      })
    })
    
    // Return the HTML response
    return HttpServerResponse.html(doc.toString())
  })
}

/**
 * Render a single message using the HTML builder
 */
function renderMessage(doc: ReturnType<typeof html>, message: any) {
  const role = message.role || 'assistant'
  const timestamp = message.timestamp 
    ? new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    : ''
  
  doc.element('div', { class: `message ${role}` }, () => {
    doc.element('div', { class: `message-block ${role}` }, () => {
      doc.element('div', { class: 'message-header' }, () => {
        doc.element('span', { class: `message-role ${role}` }, role === 'user' ? 'You' : 'Assistant')
        if (timestamp) {
          doc.element('span', { class: 'message-time' }, timestamp)
        }
      })
      
      doc.element('div', { class: 'message-body' }, () => {
        doc.element('pre', {}, message.content || '')
      })
    })
  })
}