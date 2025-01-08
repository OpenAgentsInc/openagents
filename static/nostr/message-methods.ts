import { NDKEvent } from '@nostr-dev-kit/ndk'

export class MessageMethods {
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

  private async handleNewMessage(event: NDKEvent, container: HTMLElement) {
    console.log('Handling new message:', event)
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

  private renderMessage(event: NDKEvent): HTMLElement {
    console.log('Rendering message:', event)
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

  private handleError(message: string, error: any) {
    console.error(message, error)
    this.dispatchEvent('nostr-chat:error', { message, error })
  }

  private dispatchEvent(name: string, detail: any) {
    document.dispatchEvent(new CustomEvent(name, { detail }))
  }
}