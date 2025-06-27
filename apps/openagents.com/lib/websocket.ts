// WebSocket utilities for real-time deployment progress tracking

export interface DeploymentStatus {
  id: string
  projectId: string
  status: 'pending' | 'building' | 'deploying' | 'success' | 'error'
  progress: number
  stage: string
  message?: string
  timestamp: number
  logs?: string[]
  deploymentUrl?: string
}

export interface WebSocketMessage {
  type: 'deployment_update' | 'deployment_complete' | 'deployment_error' | 'ping' | 'pong'
  data: any
}

// WebSocket connection manager
export class DeploymentWebSocket {
  private ws: WebSocket | null = null
  private reconnectAttempts = 0
  private maxReconnectAttempts = 5
  private reconnectInterval = 1000
  private listeners: Map<string, Set<(data: any) => void>> = new Map()
  private isConnected = false
  private heartbeatInterval: ReturnType<typeof setInterval> | null = null
  private currentDeploymentId: string | null = null

  constructor(
    private baseUrl: string = process.env.NEXT_PUBLIC_DEPLOYMENT_WS_URL || 
      (process.env.NODE_ENV === 'production' 
        ? 'wss://api.openagents.com/deployment-ws'
        : 'ws://localhost:8787')
  ) {
    // Don't connect immediately - wait for subscription with deploymentId
  }

  private connect(deploymentId?: string) {
    if (typeof window === 'undefined') return

    try {
      // Build URL with deploymentId parameter
      const url = new URL(this.baseUrl)
      if (deploymentId) {
        url.searchParams.set('deploymentId', deploymentId)
      }
      
      this.ws = new WebSocket(url.toString())
      
      this.ws.onopen = () => {
        console.log('WebSocket connected')
        this.isConnected = true
        this.reconnectAttempts = 0
        this.startHeartbeat()
        this.emit('connected', { connected: true })
      }

      this.ws.onmessage = (event) => {
        try {
          const message: WebSocketMessage = JSON.parse(event.data)
          this.handleMessage(message)
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error)
        }
      }

      this.ws.onclose = (event) => {
        console.log('WebSocket disconnected:', event.code, event.reason)
        this.isConnected = false
        this.stopHeartbeat()
        this.emit('disconnected', { connected: false })
        
        // Attempt to reconnect
        if (this.reconnectAttempts < this.maxReconnectAttempts && this.currentDeploymentId) {
          setTimeout(() => {
            this.reconnectAttempts++
            console.log(`Reconnecting... attempt ${this.reconnectAttempts}`)
            this.connect(this.currentDeploymentId || undefined)
          }, this.reconnectInterval * Math.pow(2, this.reconnectAttempts))
        }
      }

      this.ws.onerror = (error) => {
        console.error('WebSocket error:', error)
        this.emit('error', { error })
      }

    } catch (error) {
      console.error('Failed to create WebSocket connection:', error)
    }
  }

  private handleMessage(message: WebSocketMessage) {
    switch (message.type) {
      case 'deployment_update':
        this.emit('deploymentUpdate', message.data)
        break
      case 'deployment_complete':
        this.emit('deploymentComplete', message.data)
        break
      case 'deployment_error':
        this.emit('deploymentError', message.data)
        break
      case 'pong':
        // Heartbeat response - keep connection alive
        break
      default:
        console.warn('Unknown message type:', message.type)
    }
  }

  private startHeartbeat() {
    this.heartbeatInterval = setInterval(() => {
      if (this.isConnected && this.ws?.readyState === WebSocket.OPEN) {
        this.send({ type: 'ping', data: {} })
      }
    }, 30000) // Ping every 30 seconds
  }

  private stopHeartbeat() {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval)
      this.heartbeatInterval = null
    }
  }

  public send(message: WebSocketMessage) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message))
    } else {
      console.warn('WebSocket not connected, cannot send message')
    }
  }

  public on(event: string, callback: (data: any) => void) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set())
    }
    this.listeners.get(event)!.add(callback)
  }

  public off(event: string, callback: (data: any) => void) {
    this.listeners.get(event)?.delete(callback)
  }

  private emit(event: string, data: any) {
    this.listeners.get(event)?.forEach(callback => callback(data))
  }

  public subscribe(deploymentId: string) {
    // If not connected or connected to different deployment, reconnect
    if (!this.isConnected || this.currentDeploymentId !== deploymentId) {
      this.currentDeploymentId = deploymentId
      if (this.ws) {
        this.ws.close()
      }
      this.connect(deploymentId)
    }
  }

  public unsubscribe(deploymentId: string) {
    if (this.currentDeploymentId === deploymentId) {
      this.currentDeploymentId = null
      if (this.ws) {
        this.ws.close()
      }
    }
  }

  public close() {
    this.stopHeartbeat()
    if (this.ws) {
      this.ws.close()
      this.ws = null
    }
  }

  public get connected() {
    return this.isConnected
  }
}

// React hook for deployment WebSocket
import { useEffect, useState, useRef } from 'react'

export function useDeploymentWebSocket() {
  const [connected, setConnected] = useState(false)
  const [deploymentStatus, setDeploymentStatus] = useState<DeploymentStatus | null>(null)
  const wsRef = useRef<DeploymentWebSocket | null>(null)

  useEffect(() => {
    // Initialize WebSocket connection
    wsRef.current = new DeploymentWebSocket()

    // Set up event listeners
    wsRef.current.on('connected', () => setConnected(true))
    wsRef.current.on('disconnected', () => setConnected(false))
    
    wsRef.current.on('deploymentUpdate', (status: DeploymentStatus) => {
      setDeploymentStatus(status)
    })

    wsRef.current.on('deploymentComplete', (status: DeploymentStatus) => {
      setDeploymentStatus(status)
    })

    wsRef.current.on('deploymentError', (status: DeploymentStatus) => {
      setDeploymentStatus(status)
    })

    // Cleanup on unmount
    return () => {
      wsRef.current?.close()
    }
  }, [])

  const subscribeToDeployment = (deploymentId: string) => {
    wsRef.current?.subscribe(deploymentId)
  }

  const unsubscribeFromDeployment = (deploymentId: string) => {
    wsRef.current?.unsubscribe(deploymentId)
  }

  return {
    connected,
    deploymentStatus,
    subscribeToDeployment,
    unsubscribeFromDeployment
  }
}

// Mock WebSocket server simulation for development
export class MockDeploymentWebSocket {
  private deploymentId: string
  private onUpdate: (status: DeploymentStatus) => void
  private stages = [
    { stage: 'Initializing', progress: 0, message: 'Preparing deployment environment' },
    { stage: 'Building', progress: 20, message: 'Installing dependencies and building project' },
    { stage: 'Testing', progress: 40, message: 'Running tests and validating build' },
    { stage: 'Packaging', progress: 60, message: 'Creating deployment package' },
    { stage: 'Deploying', progress: 80, message: 'Uploading to cloud infrastructure' },
    { stage: 'Finalizing', progress: 95, message: 'Configuring DNS and SSL certificates' },
    { stage: 'Complete', progress: 100, message: 'Deployment successful!' }
  ]

  constructor(deploymentId: string, onUpdate: (status: DeploymentStatus) => void) {
    this.deploymentId = deploymentId
    this.onUpdate = onUpdate
  }

  start(projectName?: string) {
    let currentStage = 0
    const projectId = projectName || `project-${this.deploymentId.slice(0, 8)}`
    
    const updateProgress = () => {
      if (currentStage >= this.stages.length) return

      const stage = this.stages[currentStage]
      const status: DeploymentStatus = {
        id: this.deploymentId,
        projectId: projectId,
        status: currentStage === this.stages.length - 1 ? 'success' : 'building',
        progress: stage.progress,
        stage: stage.stage,
        message: stage.message,
        timestamp: Date.now(),
        logs: [`[${new Date().toISOString()}] ${stage.message}`]
      }

      // Add deployment URL on completion
      if (currentStage === this.stages.length - 1) {
        const cleanProjectId = projectId.toLowerCase().replace(/[^a-z0-9-]/g, '-')
        status.deploymentUrl = `https://${cleanProjectId}-${this.deploymentId.slice(0, 8)}.openagents.dev`
      }

      this.onUpdate(status)
      currentStage++

      if (currentStage < this.stages.length) {
        // Random delay between 1-3 seconds per stage
        const delay = 1000 + Math.random() * 2000
        setTimeout(updateProgress, delay)
      }
    }

    // Start after a short delay
    setTimeout(updateProgress, 500)
  }
}

// Global WebSocket instance for the app (optional - created on demand)
export const deploymentWebSocket = typeof window !== 'undefined' 
  ? new DeploymentWebSocket() 
  : null