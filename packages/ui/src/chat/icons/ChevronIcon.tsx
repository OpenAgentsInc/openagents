import React from 'react'
import { ChevronDown, ChevronRight } from 'lucide-react'

interface ChevronIconProps {
  direction: 'down' | 'right'
  size?: number
  style?: any
}

export const ChevronIcon = ({ direction, size = 16, style }: ChevronIconProps) => {
  const Icon = direction === 'down' ? ChevronDown : ChevronRight
  return <Icon size={size} style={style} />
}
