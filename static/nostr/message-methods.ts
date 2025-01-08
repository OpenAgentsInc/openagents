import { NDKEvent } from '@nostr-dev-kit/ndk'
import { NostrChatBase } from './base'

export class MessageMethods extends NostrChatBase {
  async handleSubmit(form: HTMLFormElement) {
    console.log('Form submitted')
    const content = new FormData(form).get('content') as string
    if (!content?.trim()) return

    try {
      if (!this.signer) {
        throw new Error('No signer available')
      }

      console.log('Creating message event...')
      const event = await this.createMessageEvent(content)
      console.log('Publishing message:', event)
      await window.ndk.publish(event)
      console.log('Message published successfully')
      form.reset()
      this.dispatchEvent('nostr-chat:message-sent', { event })
    } catch (error) {
      console.error('Failed to send message:', error)
      this.handleError('Failed to send message', error)
      // Show error to user
      const errorDiv = form.querySelector('.error-message') || document.createElement('div')
      errorDiv.className = 'error-message'
      errorDiv.textContent = 'Failed to send message. Make sure your Nostr extension is unlocked.'
      form.appendChild(errorDiv)
    }
  }

  private async createMessageEvent(content: string): Promise<NDKEvent> {
    if (!this.state.channelId) throw new Error('No channel selected')

    console.log('Creating message event with content:', content)
    const event = new NDKEvent(window.ndk)
    event.kind = 42
    event.content = content
    event.tags = [['e', this.state.channelId, '', 'root']]
    
    // Ensure the event is properly signed
    await event.sign()
    console.log('Event signed:', event)
    
    return event
  }

  setupMessagePosting(form: HTMLFormElement) {
    console.log('Setting up message form:', form)
  }
}