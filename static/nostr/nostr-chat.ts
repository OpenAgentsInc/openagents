import NDK, { NDKEvent, NDKSubscription, NDKNip07Signer } from '@nostr-dev-kit/ndk'
import { NostrChatConfig, ChatState, ChannelMetadata } from './types'
import { ChatStorage } from './storage'
import ndk from '../ndk'

declare global {
  interface Window {
    htmx: any
    nostr?: {
      getPublicKey(): Promise<string>
      signEvent(event: any): Promise<any>
    }
  }
}

class NostrChat {
  private config: NostrChatConfig
  private state: ChatState
  private storage: ChatStorage
  private templates: Map<string, HTMLTemplateElement>
  private signer: NDKNip07Signer | null = null

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
  async init() {
    await this.initializeSigner()
    this.setupHtmxExtension()
    this.loadTemplates()
  }

  private async initializeSigner() {
    try {
      if (typeof window.nostr !== 'undefined') {
        this.signer = new NDKNip07Signer()
        ndk.signer = this.signer
        console.log('NIP-07 signer initialized')
      } else {
        console.warn('No NIP-07 extension found. Message sending will be disabled.')
        this.disableMessageSending()
      }
    } catch (error) {
      console.error('Failed to initialize NIP-07 signer:', error)
      this.disableMessageSending()
    }
  }

  private disableMessageSending() {
    // Find and disable all message input forms
    document.querySelectorAll('[nostr-chat-post]').forEach(form => {
      const input = form.querySelector('input, textarea')
      const button = form.querySelector('button')
      if (input) input.setAttribute('disabled', 'true')
      if (button) button.setAttribute('disabled', 'true')
      form.setAttribute('title', 'Please install a Nostr extension (like nos2x or Alby) to send messages')
    })
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
      if (this.signer) {
        this.setupMessagePosting(element as HTMLFormElement)
      } else {
        element.setAttribute('disabled', 'true')
        element.setAttribute('title', 'Please install a Nostr extension to send messages')
      }
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
        if (!this.signer) {
          throw new Error('No signer available')
        }

        const event = await this.createMessageEvent(content)
        await ndk.publish(event)
        form.reset()
        this.dispatchEvent('nostr-chat:message-sent', { event })
      } catch (error) {
        this.handleError('Failed to send message', error)
        // Show error to user
        const errorDiv = form.querySelector('.error-message') || document.createElement('div')
        errorDiv.className = 'error-message'
        errorDiv.textContent = 'Failed to send message. Make sure your Nostr extension is unlocked.'
        form.appendChild(errorDiv)
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

  // Rest of the class implementation remains the same...
  // (keeping all the other methods unchanged)
}

// Initialize the extension
const nostrChat = new NostrChat()
nostrChat.init()