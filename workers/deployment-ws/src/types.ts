// Type definitions for deployment WebSocket service

export interface Env {
  DEPLOYMENT_SESSIONS: DurableObjectNamespace;
  INTERNAL_API_KEY?: string;
  ENVIRONMENT?: string;
}

export interface DeploymentStatus {
  id: string;
  projectId: string;
  status: 'pending' | 'building' | 'deploying' | 'success' | 'error';
  progress: number;
  stage: string;
  message?: string;
  timestamp: number;
  logs?: string[];
  deploymentUrl?: string;
}

export interface WebSocketMessage {
  type: 'deployment_update' | 'deployment_complete' | 'deployment_error' | 'ping' | 'pong';
  data?: any;
}

export interface ConnectionInfo {
  id: string;
  connectedAt: number;
  deploymentId: string;
}