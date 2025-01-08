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

  // Initialize the extension
  async init() {
    console.log('Initializing NostrChat...')
    await this.initializeSigner()
    this.setupHtmxExtension()
    this.loadTemplates()
    this.setupFormHandlers()
    await this.loadExistingChannels()
    console.log('NostrChat initialized')
  }
}

// Initialize the extension
const nostrChat = new NostrChat()
nostrChat.init()