import React from 'react'

function normalizeStyle(style: any): React.CSSProperties {
  const s: any = { ...(style || {}) }
  if (s.paddingVertical != null) {
    const v = s.paddingVertical; delete s.paddingVertical
    s.paddingTop = v; s.paddingBottom = v
  }
  if (s.paddingHorizontal != null) {
    const v = s.paddingHorizontal; delete s.paddingHorizontal
    s.paddingLeft = v; s.paddingRight = v
  }
  // If any flex props are present, ensure display:flex
  if (s.flexDirection != null || s.alignItems != null || s.justifyContent != null || s.gap != null) {
    s.display = s.display || 'flex'
  }
  return s as React.CSSProperties
}

export type ViewProps = React.HTMLAttributes<HTMLDivElement> & { testID?: string; style?: React.CSSProperties | any }
export const View: React.FC<ViewProps> = ({ children, style, testID, ...rest }) => {
  const merged = normalizeStyle(style)
  return <div data-testid={testID} style={merged} {...rest}>{children}</div>
}

export type TextProps = React.HTMLAttributes<HTMLSpanElement> & { numberOfLines?: number; testID?: string; style?: React.CSSProperties | any }
export const Text: React.FC<TextProps> = ({ children, numberOfLines, style, testID, ...rest }) => {
  const merged: React.CSSProperties = normalizeStyle(style)
  if (numberOfLines && numberOfLines > 0) {
    merged.display = (merged.display as any) || '-webkit-box'
    ;(merged as any).WebkitLineClamp = numberOfLines
    ;(merged as any).WebkitBoxOrient = 'vertical'
    merged.overflow = 'hidden'
  }
  return <span data-testid={testID} style={merged} {...rest}>{children}</span>
}

export type PressableProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  onPress?: () => void; onLongPress?: () => void; delayLongPress?: number; testID?: string; style?: React.CSSProperties | any; accessibilityRole?: string
}
export const Pressable: React.FC<PressableProps> = ({ children, onPress, onClick, onLongPress, delayLongPress = 300, style, testID, accessibilityRole, ...rest }) => {
  const timeout = React.useRef<number | null>(null)
  const handleMouseDown = () => {
    if (!onLongPress) return
    timeout.current = window.setTimeout(() => { onLongPress?.() }, delayLongPress)
  }
  const handleMouseUp = () => {
    if (timeout.current) { clearTimeout(timeout.current); timeout.current = null }
  }
  const base: React.CSSProperties = {
    background: 'transparent',
    border: 'none',
    padding: 0,
    display: 'block',
    textAlign: 'left',
    width: '100%',
    color: 'inherit',
    font: 'inherit',
    outline: 'none',
    cursor: 'pointer',
  }
  const merged = { ...base, ...normalizeStyle(style) }
  return (
    <button
      data-testid={testID}
      role={accessibilityRole as any}
      style={merged}
      onMouseDown={handleMouseDown}
      onMouseUp={handleMouseUp}
      onClick={(onClick as any) || onPress}
      {...rest as any}
    >
      {children}
    </button>
  )
}

export default { View, Text, Pressable }
