import React from 'react'

type IconProps = { name?: string; size?: number; color?: string; style?: React.CSSProperties }

export const Ionicons: React.FC<IconProps> = ({ name, size = 12, color = '#8a8f98', style }) => {
  const glyph = (() => {
    switch (String(name || '').toLowerCase()) {
      case 'time-outline':
        return '⏰'
      case 'code-slash':
        return '</>'
      case 'flash-outline':
        return '⚡'
      default:
        return '•'
    }
  })()
  return <span style={{ fontSize: size, color, lineHeight: 1, display: 'inline-block', ...style }}>{glyph}</span>
}

export default { Ionicons }

