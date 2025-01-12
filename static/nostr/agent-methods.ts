import { NDKEvent } from '@nostr-dev-kit/ndk'
import { NostrChatBase } from './base'

export interface AgentState {
  agents: Map<string, AgentData>
  instances: Map<string, InstanceData>
  plans: Map<string, PlanData>
  tasks: Map<string, TaskData>
}

export interface AgentData {
  id: string
  name: string
  description: string
  pubkey: string
  enabled: boolean
  config: any
  created_at: number
}

export interface InstanceData {
  id: string
  agent_id: string
  status: 'Starting' | 'Running' | 'Paused' | 'Stopping' | 'Stopped' | 'Error'
  created_at: number
  ended_at?: number
  memory_usage?: number
  cpu_usage?: number
}

export interface PlanData {
  id: string
  agent_id: string
  name: string
  description: string
  status: 'Created' | 'InProgress' | 'Completed' | 'Failed' | 'Cancelled'
  task_ids: string[]
  created_at: number
  ended_at?: number
  metadata: any
}

export interface TaskData {
  id: string
  plan_id: string
  instance_id: string
  task_type: string
  status: 'Pending' | 'Scheduled' | 'Running' | 'Completed' | 'Failed' | 'Cancelled'
  priority: number
  input: any
  output?: any
  created_at: number
  started_at?: number
  ended_at?: number
  error?: string
}

export class NostrAgentMethods extends NostrChatBase {
  protected agentState: AgentState

  constructor() {
    super()
    this.agentState = {
      agents: new Map(),
      instances: new Map(),
      plans: new Map(),
      tasks: new Map()
    }
  }

  // Agent Creation - Kind 30001
  async createAgent(data: Partial<AgentData>) {
    if (!this.signer) {
      throw new Error('No signer available')
    }

    const event = new NDKEvent(this.api)
    event.kind = 30001
    event.content = JSON.stringify(data)
    event.tags = [
      ['d', 'agent_creation'],
      ['p', await this.signer.getPublicKey()]
    ]

    try {
      await event.sign(this.signer)
      await event.publish()
      this.dispatchEvent('agent:created', { agentId: data.id })
    } catch (error) {
      this.handleError('Failed to create agent', error)
    }
  }

  // Instance Status Update - Kind 30004
  async updateInstanceStatus(instanceId: string, status: InstanceData['status'], metrics?: { memory_usage?: number, cpu_usage?: number }) {
    if (!this.signer) {
      throw new Error('No signer available')
    }

    const event = new NDKEvent(this.api)
    event.kind = 30004
    event.content = JSON.stringify({ status, ...metrics })
    event.tags = [
      ['d', instanceId],
      ['p', await this.signer.getPublicKey()],
      ['t', 'instance_state']
    ]

    try {
      await event.sign(this.signer)
      await event.publish()
      this.dispatchEvent('instance:updated', { instanceId, status, metrics })
    } catch (error) {
      this.handleError('Failed to update instance status', error)
    }
  }

  // Plan Creation/Update - Kind 30002
  async updatePlan(planData: Partial<PlanData>) {
    if (!this.signer) {
      throw new Error('No signer available')
    }

    const event = new NDKEvent(this.api)
    event.kind = 30002
    event.content = JSON.stringify(planData)
    event.tags = [
      ['d', planData.id!],
      ['p', await this.signer.getPublicKey()],
      ['t', 'plan_update']
    ]

    try {
      await event.sign(this.signer)
      await event.publish()
      this.dispatchEvent('plan:updated', { planId: planData.id })
    } catch (error) {
      this.handleError('Failed to update plan', error)
    }
  }

  // Task State Update - Kind 30003
  async updateTaskState(taskData: Partial<TaskData>) {
    if (!this.signer) {
      throw new Error('No signer available')
    }

    const event = new NDKEvent(this.api)
    event.kind = 30003
    event.content = JSON.stringify(taskData)
    event.tags = [
      ['d', taskData.id!],
      ['p', await this.signer.getPublicKey()],
      ['t', 'task_state']
    ]

    try {
      await event.sign(this.signer)
      await event.publish()
      this.dispatchEvent('task:updated', { taskId: taskData.id })
    } catch (error) {
      this.handleError('Failed to update task state', error)
    }
  }

  // Task Output - Kind 1002
  async publishTaskOutput(taskId: string, output: any, error?: string) {
    if (!this.signer) {
      throw new Error('No signer available')
    }

    const event = new NDKEvent(this.api)
    event.kind = 1002
    event.content = JSON.stringify({ output, error })
    event.tags = [
      ['e', taskId],
      ['p', await this.signer.getPublicKey()],
      ['t', 'task_output']
    ]

    try {
      await event.sign(this.signer)
      await event.publish()
      this.dispatchEvent('task:output', { taskId, output, error })
    } catch (error) {
      this.handleError('Failed to publish task output', error)
    }
  }

  // Event Handlers
  protected handleAgentEvent(event: NDKEvent) {
    try {
      const data = JSON.parse(event.content)
      const type = event.tags.find(t => t[0] === 't')?.[1]

      switch (event.kind) {
        case 30001: // Agent Creation/Update
          if (type === 'agent_creation') {
            this.agentState.agents.set(data.id, data)
            this.dispatchEvent('agent:updated', { agent: data })
          }
          break

        case 30002: // Plan Update
          if (type === 'plan_update') {
            this.agentState.plans.set(data.id, data)
            this.dispatchEvent('plan:updated', { plan: data })
          }
          break

        case 30003: // Task State
          if (type === 'task_state') {
            this.agentState.tasks.set(data.id, data)
            this.dispatchEvent('task:updated', { task: data })
          }
          break

        case 30004: // Instance State
          if (type === 'instance_state') {
            const instanceId = event.tags.find(t => t[0] === 'd')?.[1]
            if (instanceId) {
              this.agentState.instances.set(instanceId, {
                ...this.agentState.instances.get(instanceId),
                ...data
              })
              this.dispatchEvent('instance:updated', { 
                instanceId, 
                state: this.agentState.instances.get(instanceId)
              })
            }
          }
          break

        case 1002: // Task Output
          if (type === 'task_output') {
            const taskId = event.tags.find(t => t[0] === 'e')?.[1]
            if (taskId) {
              const task = this.agentState.tasks.get(taskId)
              if (task) {
                task.output = data.output
                task.error = data.error
                this.agentState.tasks.set(taskId, task)
                this.dispatchEvent('task:output', { 
                  taskId,
                  output: data.output,
                  error: data.error
                })
              }
            }
          }
          break
      }
    } catch (error) {
      this.handleError('Failed to handle agent event', error)
    }
  }

  // Subscribe to agent events
  async subscribeToAgentEvents(pubkey: string) {
    if (!this.api) {
      throw new Error('NDK not initialized')
    }

    const sub = this.api.subscribe([
      { kinds: [30001, 30002, 30003, 30004, 1002], authors: [pubkey] }
    ])

    sub.on('event', (event: NDKEvent) => {
      this.handleAgentEvent(event)
    })

    return sub
  }
}