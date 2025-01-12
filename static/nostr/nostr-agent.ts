import NDK, { NDKNip07Signer } from '@nostr-dev-kit/ndk'
import { NostrAgentMethods } from './agent-methods'

declare global {
  interface Window {
    NDK: typeof NDK
    NostrAgent: typeof NostrAgent
    nostr?: any
  }
}

export class NostrAgent extends NostrAgentMethods {
  constructor() {
    super()
    this.initializeNostrAgent()
  }

  private async initializeNostrAgent() {
    try {
      // Initialize NDK
      this.api = new NDK({
        explicitRelayUrls: this.config.defaultRelays
      })

      // Try to get NIP-07 signer
      try {
        if (!window.nostr) {
          throw new Error('No NIP-07 provider found. Please install a Nostr signer extension.')
        }
        this.signer = new NDKNip07Signer()
        await this.api.connect()
      } catch (error) {
        this.handleError('Failed to initialize NIP-07 signer', error)
        return
      }

      // Load templates
      const agentItemTemplate = document.querySelector<HTMLTemplateElement>('#agent-item-template')
      if (agentItemTemplate) {
        this.templates.set('agent-item', agentItemTemplate)
      }

      // Initialize HTMX extension
      this.initializeHtmxExtension()

      // Subscribe to agent events for the current user
      if (this.signer) {
        const pubkey = await this.signer.getPublicKey()
        await this.subscribeToAgentEvents(pubkey)
      }

    } catch (error) {
      this.handleError('Failed to initialize NostrAgent', error)
    }
  }

  private initializeHtmxExtension() {
    // @ts-ignore
    htmx.defineExtension('nostr-agent', {
      onEvent: async (name: string, evt: any) => {
        if (!this.signer) {
          this.handleError('No signer available', null)
          return
        }

        try {
          const elt = evt.detail.elt
          const action = elt.getAttribute('nostr-action')
          const agentId = elt.getAttribute('data-agent-id')

          switch (action) {
            case 'create':
              const formData = new FormData(elt)
              const agentData = {
                id: crypto.randomUUID(),
                name: formData.get('name'),
                description: formData.get('description'),
                config: JSON.parse(formData.get('config') as string),
                memory_limit: parseInt(formData.get('memory_limit') as string),
                cpu_limit: parseInt(formData.get('cpu_limit') as string),
                created_at: Math.floor(Date.now() / 1000)
              }
              await this.createAgent(agentData)
              break

            case 'start':
              await this.updateInstanceStatus(agentId, 'Starting')
              break

            case 'stop':
              await this.updateInstanceStatus(agentId, 'Stopping')
              break

            case 'delete':
              // Send deletion event
              const event = {
                kind: 30001,
                content: JSON.stringify({ id: agentId, deleted: true }),
                tags: [
                  ['d', 'agent_deletion'],
                  ['p', await this.signer.getPublicKey()]
                ]
              }
              await this.api.publish(event)
              break
          }
        } catch (error) {
          this.handleError('Failed to handle HTMX event', error)
        }
      }
    })
  }

  // Event Handlers
  protected override handleAgentEvent(event: any) {
    super.handleAgentEvent(event)

    // Update UI based on event type
    try {
      const data = JSON.parse(event.content)
      const type = event.tags.find((t: string[]) => t[0] === 't')?.[1]
      const template = this.templates.get('agent-item')

      if (!template) {
        return
      }

      switch (event.kind) {
        case 30001: // Agent Creation/Update
          if (type === 'agent_creation') {
            const agentList = document.getElementById('agent-items')
            if (agentList) {
              const clone = template.content.cloneNode(true) as DocumentFragment
              this.replaceTemplateVariables(clone as unknown as HTMLElement, data)
              agentList.appendChild(clone)
            }
          } else if (type === 'agent_deletion') {
            const agentElement = document.querySelector(`[data-agent-id="${data.id}"]`)
            agentElement?.remove()
          }
          break

        case 30004: // Instance State
          const instanceId = event.tags.find((t: string[]) => t[0] === 'd')?.[1]
          if (instanceId) {
            const agentElement = document.querySelector(`[data-agent-id="${instanceId}"]`)
            if (agentElement) {
              const statusElement = agentElement.querySelector('.status-badge')
              const metricsElement = agentElement.querySelector('.agent-metrics')
              
              if (statusElement) {
                statusElement.textContent = data.status
                statusElement.className = `status-badge ${data.status.toLowerCase()}`
              }

              if (metricsElement && data.memory_usage && data.cpu_usage) {
                metricsElement.innerHTML = `
                  <div>Memory: ${data.memory_usage}MB / ${data.memory_limit}MB</div>
                  <div>CPU: ${data.cpu_usage}ms / ${data.cpu_limit}ms</div>
                `
              }
            }
          }
          break
      }
    } catch (error) {
      this.handleError('Failed to update UI', error)
    }
  }
}

// Initialize and expose to window
window.NostrAgent = NostrAgent