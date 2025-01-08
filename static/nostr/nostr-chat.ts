import NDK, { NDKEvent, NDKSubscription } from '@nostr-dev-kit/ndk'
import { NostrChatConfig, ChatState, ChannelMetadata } from './types'
import { ChatStorage } from './storage'
import ndk from '../ndk'

declare global {
  interface Window {
    htmx: any
  }
}

class NostrChat {
  private config: NostrChatConfig
  private state: ChatState
  private storage: ChatStorage
  private templates: Map<string, HTMLTemplateElement>

  constructor() {
    this.config = {
      defaultRelays: [
        'wss://nostr-pub.wellorder.net',
        'wss://nostr.mom',
        'wss://relay.nostr.band'
      ],
      messageTemplate: '#message-template',
      autoScroll: true,
      moderationEnabled: true,
      pollInterval: 5000,
      messageLimit: 50
    }
    this.storage = new ChatStorage()
    this.templates = new Map()
    this.state = {
      messages: new Map(),
      hiddenMessages: new Set(),
      mutedUsers: new Set(),
      moderationActions: []
    }
  }

  // Initialize the extension
  init() {
    this.setupHtmxExtension()
    this.loadTemplates()
  }

  private setupHtmxExtension() {
    window.htmx.defineExtension('nostr-chat', {
      onEvent: (name: string, evt: CustomEvent) => {
        switch (name) {
          case 'htmx:afterProcessNode':
            this.processElement(evt.target as HTMLElement)
            break
          case 'htmx:beforeCleanupElement':
            this.cleanupElement(evt.target as HTMLElement)
            break
        }
      }
    })
  }

  private async processElement(element: HTMLElement) {
    // Channel subscription
    const channelId = element.getAttribute('nostr-chat-channel')
    if (channelId) {
      await this.subscribeToChannel(channelId, element)
    }

    // Message posting
    if (element.getAttribute('nostr-chat-post')) {
      this.setupMessagePosting(element as HTMLFormElement)
    }

    // Moderation controls
    if (this.config.moderationEnabled) {
      const hideId = element.getAttribute('nostr-chat-hide')
      if (hideId) {
        this.setupHideButton(element, hideId)
      }

      const mutePubkey = element.getAttribute('nostr-chat-mute')
      if (mutePubkey) {
        this.setupMuteButton(element, mutePubkey)
      }
    }
  }

  private cleanupElement(element: HTMLElement) {
    const channelId = element.getAttribute('nostr-chat-channel')
    if (channelId && this.state.subscription) {
      this.state.subscription.stop()
      this.state.subscription = undefined
    }
  }

  // Channel Operations
  private async subscribeToChannel(channelId: string, element: HTMLElement) {
    this.state.channelId = channelId

    // Load cached metadata
    const cached = this.storage.getChannelMetadata(channelId)
    if (cached) {
      this.renderChannelMetadata(cached, element)
    }

    // Subscribe to channel messages
    const sub = ndk.subscribe({
      kinds: [42], // channel messages
      '#e': [channelId],
    }, { closeOnEose: false })

    sub.on('event', (event: NDKEvent) => {
      this.handleNewMessage(event, element)
    })

    this.state.subscription = sub
    sub.start()

    // Also fetch channel metadata
    const metadataSub = ndk.subscribe({
      kinds: [41], // channel metadata
      '#e': [channelId],
    })

    metadataSub.on('event', (event: NDKEvent) => {
      const metadata = JSON.parse(event.content)
      this.storage.cacheChannelMetadata(channelId, metadata)
      this.renderChannelMetadata(metadata, element)
    })
  }

  // Message Operations
  private setupMessagePosting(form: HTMLFormElement) {
    form.addEventListener('submit', async (e) => {
      e.preventDefault()
      const content = new FormData(form).get('content') as string
      if (!content?.trim()) return

      try {
        const event = await this.createMessageEvent(content)
        await ndk.publish(event)
        form.reset()
        this.dispatchEvent('nostr-chat:message-sent', { event })
      } catch (error) {
        this.handleError('Failed to send message', error)
      }
    })
  }

  private async createMessageEvent(content: string): Promise<NDKEvent> {
    if (!this.state.channelId) throw new Error('No channel selected')

    const event = new NDKEvent()
    event.kind = 42
    event.content = content
    event.tags = [['e', this.state.channelId, '', 'root']]
    
    return event
  }

  // Moderation
  private setupHideButton(button: HTMLElement, messageId: string) {
    button.addEventListener('click', () => {
      const reason = button.getAttribute('nostr-chat-reason')
      this.storage.hideMessage(messageId, reason)
      this.dispatchEvent('nostr-chat:message-hidden', { messageId, reason })
    })
  }

  private setupMuteButton(button: HTMLElement, pubkey: string) {
    button.addEventListener('click', () => {
      const reason = button.getAttribute('nostr-chat-reason')
      this.storage.muteUser(pubkey, reason)
      this.dispatchEvent('nostr-chat:user-muted', { pubkey, reason })
    })
  }

  // Template Handling
  private loadTemplates() {
    document.querySelectorAll('template[id]').forEach(template => {
      this.templates.set(template.id, template as HTMLTemplateElement)
    })
  }

  private renderMessage(event: NDKEvent): HTMLElement {
    const template = this.templates.get(this.config.messageTemplate?.slice(1) || 'message-template')
    if (!template) throw new Error('Message template not found')

    const clone = template.content.cloneNode(true) as HTMLElement
    // Replace template variables
    const data = {
      id: event.id,
      pubkey: event.pubkey,
      pubkey_short: event.pubkey.slice(0, 8),
      content: event.content,
      created_at: event.created_at,
      formatted_time: new Date(event.created_at * 1000).toLocaleString()
    }

    this.replaceTemplateVariables(clone, data)
    return clone
  }

  private renderChannelMetadata(metadata: ChannelMetadata, element: HTMLElement) {
    const template = this.templates.get('channel-metadata-template')
    if (!template) return

    const clone = template.content.cloneNode(true) as HTMLElement
    this.replaceTemplateVariables(clone, metadata)
    
    const target = element.querySelector('[data-channel-metadata]')
    if (target) {
      target.innerHTML = ''
      target.appendChild(clone)
    }
  }

  private replaceTemplateVariables(element: HTMLElement, data: Record<string, any>) {
    const walker = document.createTreeWalker(
      element,
      NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT,
      null
    )

    let node
    while (node = walker.nextNode()) {
      if (node.nodeType === Node.TEXT_NODE) {
        node.textContent = node.textContent?.replace(
          /\{\{(\w+)\}\}/g,
          (_, key) => data[key] || ''
        )
      } else if (node instanceof Element) {
        Array.from(node.attributes).forEach(attr => {
          attr.value = attr.value.replace(
            /\{\{(\w+)\}\}/g,
            (_, key) => data[key] || ''
          )
        })
      }
    }
  }

  // Event Handling
  private async handleNewMessage(event: NDKEvent, container: HTMLElement) {
    if (this.storage.isMessageHidden(event.id) || 
        this.storage.isUserMuted(event.pubkey)) {
      return
    }

    this.state.messages.set(event.id, event)
    const rendered = this.renderMessage(event)
    
    const messagesContainer = container.querySelector('[data-messages]')
    if (messagesContainer) {
      messagesContainer.insertAdjacentElement('beforeend', rendered)
      if (this.config.autoScroll) {
        messagesContainer.scrollTop = messagesContainer.scrollHeight
      }
    }
  }

  private handleError(message: string, error: any) {
    console.error(message, error)
    this.dispatchEvent('nostr-chat:error', { message, error })
  }

  private dispatchEvent(name: string, detail: any) {
    document.dispatchEvent(new CustomEvent(name, { detail }))
  }
}

// Initialize the extension
const nostrChat = new NostrChat()
nostrChat.init()