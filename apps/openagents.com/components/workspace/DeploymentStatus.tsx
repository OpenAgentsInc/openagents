'use client'

import React from 'react'
import { CheckCircle, Circle, Loader, XCircle, ExternalLink } from 'lucide-react'
import { cx } from '@arwes/react'

interface DeploymentStep {
  id: string
  label: string
  status: 'pending' | 'in-progress' | 'completed' | 'error'
  duration?: string
}

interface DeploymentStatusProps {
  status: 'idle' | 'generating' | 'deploying' | 'complete' | 'error'
  deploymentUrl?: string
  className?: string
}

export function DeploymentStatus({ status, deploymentUrl, className }: DeploymentStatusProps) {
  const getSteps = (): DeploymentStep[] => {
    const baseSteps = [
      { id: 'generate', label: 'Generating code', status: 'pending' as const },
      { id: 'build', label: 'Building project', status: 'pending' as const },
      { id: 'deploy', label: 'Deploying to cloud', status: 'pending' as const },
      { id: 'verify', label: 'Verifying deployment', status: 'pending' as const }
    ]

    switch (status) {
      case 'generating':
        baseSteps[0].status = 'in-progress'
        return baseSteps

      case 'deploying':
        baseSteps[0].status = 'completed'
        baseSteps[0].duration = '1.2s'
        baseSteps[1].status = 'completed'
        baseSteps[1].duration = '3.4s'
        baseSteps[2].status = 'in-progress'
        return baseSteps

      case 'complete':
        return baseSteps.map((step, index) => ({
          ...step,
          status: 'completed' as const,
          duration: ['1.2s', '3.4s', '2.1s', '0.8s'][index]
        }))

      case 'error':
        baseSteps[0].status = 'completed'
        baseSteps[0].duration = '1.2s'
        baseSteps[1].status = 'error'
        return baseSteps

      default:
        return baseSteps
    }
  }

  const steps = getSteps()

  const getStatusIcon = (stepStatus: DeploymentStep['status']) => {
    switch (stepStatus) {
      case 'completed':
        return <CheckCircle size={16} className="text-green-400" />
      case 'in-progress':
        return <Loader size={16} className="text-cyan-400 animate-spin" />
      case 'error':
        return <XCircle size={16} className="text-red-400" />
      default:
        return <Circle size={16} className="text-gray-600" />
    }
  }

  const getStatusText = () => {
    switch (status) {
      case 'generating':
        return 'Generating your project...'
      case 'deploying':
        return 'Deploying to production...'
      case 'complete':
        return 'Deployment successful!'
      case 'error':
        return 'Deployment failed'
      default:
        return 'Ready to deploy'
    }
  }

  const getStatusColor = () => {
    switch (status) {
      case 'generating':
      case 'deploying':
        return 'text-cyan-400'
      case 'complete':
        return 'text-green-400'
      case 'error':
        return 'text-red-400'
      default:
        return 'text-gray-400'
    }
  }

  return (
    <div className={cx('bg-black/50 border border-cyan-900/30 rounded-lg p-4', className)}>
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-cyan-500 font-mono text-sm uppercase tracking-wider">
          Deployment Status
        </h3>
        {status === 'complete' && deploymentUrl && (
          <a
            href={deploymentUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1 text-cyan-400 hover:text-cyan-300 text-xs font-mono transition-colors"
          >
            View Live <ExternalLink size={12} />
          </a>
        )}
      </div>

      {/* Status Message */}
      <div className={cx('text-sm mb-6', getStatusColor())} style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>
        {getStatusText()}
      </div>

      {/* Progress Steps */}
      <div className="space-y-3">
        {steps.map((step, index) => (
          <div key={step.id} className="flex items-center gap-3">
            {getStatusIcon(step.status)}
            
            <div className="flex-1">
              <div
                className={cx(
                  'text-sm',
                  step.status === 'completed' ? 'text-gray-300' :
                  step.status === 'in-progress' ? 'text-cyan-300' :
                  step.status === 'error' ? 'text-red-300' :
                  'text-gray-500'
                )}
                style={{ fontFamily: 'var(--font-titillium), sans-serif' }}
              >
                {step.label}
              </div>
              
              {step.duration && step.status === 'completed' && (
                <div className="text-xs text-gray-500 mt-1" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
                  Completed in {step.duration}
                </div>
              )}
            </div>

            {/* Progress Bar */}
            {step.status === 'in-progress' && (
              <div className="w-24 h-1 bg-gray-800 rounded-full overflow-hidden">
                <div className="h-full bg-cyan-400 rounded-full animate-pulse" style={{ width: '60%' }} />
              </div>
            )}
          </div>
        ))}
      </div>

      {/* Total Time for Complete Status */}
      {status === 'complete' && (
        <div className="mt-4 pt-4 border-t border-cyan-900/30">
          <div className="text-xs text-gray-400" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
            Total deployment time: 7.5s
          </div>
        </div>
      )}

      {/* Error Details */}
      {status === 'error' && (
        <div className="mt-4 pt-4 border-t border-red-900/30">
          <div className="text-xs text-red-400 bg-red-950/20 border border-red-900/30 rounded p-2" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
            Build failed: Missing dependency 'react-dom'
          </div>
          <button className="mt-2 text-xs text-cyan-400 hover:text-cyan-300 font-mono uppercase tracking-wider">
            Retry Deployment
          </button>
        </div>
      )}
    </div>
  )
}