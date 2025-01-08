import NDK, { NDKEvent, NDKSubscription, NDKNip07Signer } from '@nostr-dev-kit/ndk'
import { NostrChatConfig, ChatState, ChannelMetadata, CreateChannelData } from './types'
import { ChatStorage } from './storage'

declare global {
  interface Window {
    htmx: any
    nostr?: {
      getPublicKey(): Promise<string>
      signEvent(event: any): Promise<any>
    }
    ndk: NDK
  }
}

class NostrChat {
  private config: NostrChatConfig
  private state: ChatState
  private storage: ChatStorage
  private templates: Map<string, HTMLTemplateElement>
  private signer: NDKNip07Signer | null = null
  private api: any

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
      moderationActions: [],
      channels: new Map()
    }

    // Bind methods
    this.loadTemplates = this.loadTemplates.bind(this)
    this.setupHtmxExtension = this.setupHtmxExtension.bind(this)
    this.processElement = this.processElement.bind(this)
    this.cleanupElement = this.cleanupElement.bind(this)
    this.handleNewMessage = this.handleNewMessage.bind(this)
    this.handleSubmit = this.handleSubmit.bind(this)
    this.handleCreateChannel = this.handleCreateChannel.bind(this)
  }

  async init() {
    console.log('Initializing NostrChat...')
    await this.initializeSigner()
    this.setupHtmxExtension()
    this.loadTemplates()
    this.setupFormHandlers()
    await this.loadExistingChannels()
    console.log('NostrChat initialized')
  }

  private async initializeSigner() {
    try {
      if (typeof window.nostr !== 'undefined') {
        console.log('Found nostr provider, initializing signer...')
        this.signer = new NDKNip07Signer()
        window.ndk.signer = this.signer
        // Test the signer
        const pubkey = await this.signer.user().then(user => user.pubkey)
        console.log('NIP-07 signer initialized with pubkey:', pubkey)
      } else {
        console.warn('No NIP-07 extension found. Message sending will be disabled.')
        this.disableMessageSending()
      }
    } catch (error) {
      console.error('Failed to initialize NIP-07 signer:', error)
      this.disableMessageSending()
    }
  }

  private setupHtmxExtension() {
    console.log('Setting up HTMX extension...')
    window.htmx.defineExtension('nostr-chat', {
      init: (apiRef: any) => {
        console.log('HTMX extension initialized with API')
        this.api = apiRef
      },
      onEvent: (name: string, evt: CustomEvent) => {
        console.log('HTMX event:', name)
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

  private loadTemplates() {
    document.querySelectorAll('template[id]').forEach(template => {
      console.log('Loading template:', template.id)
      this.templates.set(template.id, template as HTMLTemplateElement)
    })
  }

  private setupFormHandlers() {
    document.addEventListener('submit', async (e) => {
      const form = e.target as HTMLFormElement
      e.preventDefault()

      if (form.getAttribute('nostr-chat-create')) {
        await this.handleCreateChannel(form)
      } else if (form.getAttribute('nostr-chat-post')) {
        await this.handleSubmit(form)
      }
    })
  }

  private disableMessageSending() {
    document.querySelectorAll('[nostr-chat-post]').forEach(form => {
      const input = form.querySelector('input, textarea')
      const button = form.querySelector('button')
      if (input) input.setAttribute('disabled', 'true')
      if (button) button.setAttribute('disabled', 'true')
      form.setAttribute('title', 'Please install a Nostr extension (like nos2x or Alby) to send messages')
    })
  }

  private async loadExistingChannels() {
    console.log('Loading existing channels...')
    if (!this.signer) return

    const sub = window.ndk.subscribe({
      kinds: [40], // channel creation events
      authors: [await this.signer.user().then(user => user.pubkey)]
    })

    sub.on('event', (event: NDKEvent) => {
      try {
        const metadata = JSON.parse(event.content)
        this.state.channels.set(event.id, metadata)
        this.renderChannelItem(event.id, metadata)
      } catch (error) {
        console.error('Failed to parse channel metadata:', error)
      }
    })

    sub.start()
  }
}

// Initialize the extension
const nostrChat = new NostrChat()
nostrChat.init()