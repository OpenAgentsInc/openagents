// ... (keep all imports)

// Add this after setupFormHandlers() in the NostrChat class:

private async handleCreateChannel(form: HTMLFormElement) {
    console.log('Creating channel...')
    const formData = new FormData(form)
    const channelData: CreateChannelData = {
        name: formData.get('name') as string,
        about: formData.get('about') as string,
        picture: formData.get('picture') as string || undefined,
        relays: this.config.defaultRelays
    }

    try {
        if (!this.signer) {
            throw new Error('No signer available')
        }

        // Create kind 40 event for channel creation
        const event = new NDKEvent(window.ndk)
        event.kind = 40
        event.content = JSON.stringify(channelData)
        
        console.log('Signing channel creation event...')
        await event.sign()
        
        console.log('Publishing channel...')
        await window.ndk.publish(event)
        
        console.log('Channel created:', event)
        
        // Add to local state
        this.state.channels.set(event.id, channelData)
        
        // Add to channel list UI
        this.renderChannelItem(event.id, channelData)
        
        // Clear form
        form.reset()
        
        // Show success message
        const successDiv = document.createElement('div')
        successDiv.className = 'success-message'
        successDiv.textContent = 'Channel created successfully!'
        form.appendChild(successDiv)
        setTimeout(() => successDiv.remove(), 3000)

        // Switch to the new channel
        this.selectChannel(event.id)

    } catch (error) {
        console.error('Failed to create channel:', error)
        const errorDiv = form.querySelector('.error-message') || document.createElement('div')
        errorDiv.className = 'error-message'
        errorDiv.textContent = 'Failed to create channel. Make sure your Nostr extension is unlocked.'
        form.appendChild(errorDiv)
    }
}

private renderChannelItem(channelId: string, metadata: ChannelMetadata) {
    const template = this.templates.get('channel-item-template')
    if (!template) {
        console.error('Channel item template not found')
        return
    }

    const clone = template.content.cloneNode(true) as HTMLElement
    const data = {
        id: channelId,
        name: metadata.name,
        about: metadata.about
    }

    this.replaceTemplateVariables(clone, data)
    
    const channelList = document.getElementById('channel-items')
    if (channelList) {
        const item = clone.querySelector('.channel-item')
        if (item) {
            item.addEventListener('click', () => this.selectChannel(channelId))
            channelList.appendChild(item)
        }
    }
}

private async selectChannel(channelId: string) {
    console.log('Selecting channel:', channelId)
    
    // Update UI
    document.querySelectorAll('.channel-item').forEach(item => {
        item.classList.remove('active')
        if (item.getAttribute('data-channel-id') === channelId) {
            item.classList.add('active')
        }
    })

    // Show chat interface
    const chatInterface = document.getElementById('chat-interface')
    if (chatInterface) {
        chatInterface.style.display = 'block'
    }

    // Set channel ID and subscribe
    this.state.channelId = channelId
    const container = document.querySelector('[data-messages]')?.parentElement
    if (container) {
        await this.subscribeToChannel(channelId, container)
    }
}

// Update setupFormHandlers():
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

// Update constructor to initialize channels Map:
constructor() {
    // ... (keep existing initialization)
    this.state = {
        messages: new Map(),
        hiddenMessages: new Set(),
        mutedUsers: new Set(),
        moderationActions: [],
        channels: new Map() // Add this line
    }
    
    // ... (keep rest of constructor)
}

// Add method to load existing channels:
private async loadExistingChannels() {
    console.log('Loading existing channels...')
    const sub = window.ndk.subscribe({
        kinds: [40], // channel creation events
        authors: [await this.signer?.user().then(user => user.pubkey)].filter(Boolean)
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

// Update init() to load existing channels:
async init() {
    console.log('Initializing NostrChat...')
    await this.initializeSigner()
    this.setupHtmxExtension()
    this.loadTemplates()
    this.setupFormHandlers()
    await this.loadExistingChannels() // Add this line
    console.log('NostrChat initialized')
}

// ... (keep rest of the class implementation)