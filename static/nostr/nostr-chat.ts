import { NDKEvent, NDKNip07Signer } from '@nostr-dev-kit/ndk'
import { NostrChatBase } from './base'
import { ChannelMethods } from './channel-methods'
import { MessageMethods } from './message-methods'

class NostrChat extends NostrChatBase {
  private channelMethods: ChannelMethods
  private messageMethods: MessageMethods

  constructor() {
    super()
    // Pass 'this' as the shared state to child classes
    this.channelMethods = new ChannelMethods(this)
    this.messageMethods = new MessageMethods(this)

    // Bind methods
    this.loadTemplates = this.loadTemplates.bind(this)
    this.setupHtmxExtension = this.setupHtmxExtension.bind(this)
    this.processElement = this.processElement.bind(this)
    this.cleanupElement = this.cleanupElement.bind(this)
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
        await this.channelMethods.handleCreateChannel(form)
      } else if (form.getAttribute('nostr-chat-post')) {
        await this.messageMethods.handleSubmit(form)
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
        this.channelMethods.renderChannelItem(event.id, metadata)
      } catch (error) {
        console.error('Failed to parse channel metadata:', error)
      }
    })

    sub.start()
  }

  private async processElement(element: HTMLElement) {
    console.log('Processing element:', element)
    
    // Channel subscription
    const channelId = element.getAttribute('nostr-chat-channel')
    if (channelId) {
      console.log('Found channel ID:', channelId)
      await this.channelMethods.subscribeToChannel(channelId, element)
    }

    // Message posting
    if (element.getAttribute('nostr-chat-post')) {
      console.log('Setting up message posting form')
      if (this.signer) {
        this.messageMethods.setupMessagePosting(element as HTMLFormElement)
      } else {
        element.setAttribute('disabled', 'true')
        element.setAttribute('title', 'Please install a Nostr extension to send messages')
      }
    }
  }

  private cleanupElement(element: HTMLElement) {
    const channelId = element.getAttribute('nostr-chat-channel')
    if (channelId && this.state.subscription) {
      console.log('Cleaning up subscription for channel:', channelId)
      this.state.subscription.stop()
      this.state.subscription = undefined
    }
  }

  // Expose state to child classes
  getSigner() { return this.signer }
  getState() { return this.state }
  getConfig() { return this.config }
  getTemplates() { return this.templates }
  getStorage() { return this.storage }
}

// Initialize the extension
const nostrChat = new NostrChat()
nostrChat.init()