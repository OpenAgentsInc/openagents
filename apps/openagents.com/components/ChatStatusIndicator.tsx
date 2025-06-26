import React from 'react'
import { Text } from '@arwes/react'
import { Clock, Play, CheckCircle, AlertCircle } from 'lucide-react'
import type { ChatStatus } from './ChatInput'

interface ChatStatusIndicatorProps {
  status: ChatStatus
}

export const ChatStatusIndicator: React.FC<ChatStatusIndicatorProps> = ({ status }) => {
  const getStatusInfo = () => {
    switch (status) {
      case 'submitted':
        return { icon: Clock, color: 'text-yellow-400', text: 'Submitted' }
      case 'streaming':
        return { icon: Play, color: 'text-blue-400 animate-pulse', text: 'Streaming' }
      case 'ready':
        return { icon: CheckCircle, color: 'text-green-400', text: 'Ready' }
      case 'error':
        return { icon: AlertCircle, color: 'text-red-400', text: 'Error' }
    }
  }

  const { icon: Icon, color, text } = getStatusInfo()

  return (
    <div className="flex items-center gap-2 text-xs">
      <Icon size={12} className={color} />
      <Text className={`${color} font-mono`}>{text}</Text>
    </div>
  )
}