import { NDKEvent } from '@nostr-dev-kit/ndk'
import { ChannelMetadata, CreateChannelData } from './types'

export class ChannelMethods {
  async handleCreateChannel(form: HTMLFormElement) {
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

  renderChannelItem(channelId: string, metadata: ChannelMetadata) {
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

  async selectChannel(channelId: string) {
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

  private async subscribeToChannel(channelId: string, element: HTMLElement) {
    console.log('Subscribing to channel:', channelId)
    this.state.channelId = channelId

    // Load cached metadata
    const cached = this.storage.getChannelMetadata(channelId)
    if (cached) {
      this.renderChannelMetadata(cached, element)
    }

    // Subscribe to channel messages
    const sub = window.ndk.subscribe({
      kinds: [42], // channel messages
      '#e': [channelId],
    }, { closeOnEose: false })

    sub.on('event', (event: NDKEvent) => {
      console.log('Received channel message:', event)
      this.handleNewMessage(event, element)
    })

    this.state.subscription = sub
    sub.start()

    // Also fetch channel metadata
    const metadataSub = window.ndk.subscribe({
      kinds: [41], // channel metadata
      '#e': [channelId],
    })

    metadataSub.on('event', (event: NDKEvent) => {
      console.log('Received channel metadata:', event)
      const metadata = JSON.parse(event.content)
      this.storage.cacheChannelMetadata(channelId, metadata)
      this.renderChannelMetadata(metadata, element)
    })
  }
}