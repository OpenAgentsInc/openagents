import React from 'react'

export type ViewProps = React.HTMLAttributes<HTMLDivElement> & { testID?: string; style?: React.CSSProperties | any }
export const View: React.FC<ViewProps> = ({ children, style, testID, ...rest }) => {
  return <div data-testid={testID} style={style as React.CSSProperties} {...rest}>{children}</div>
}

export type TextProps = React.HTMLAttributes<HTMLSpanElement> & { numberOfLines?: number; testID?: string; style?: React.CSSProperties | any }
export const Text: React.FC<TextProps> = ({ children, numberOfLines, style, testID, ...rest }) => {
  const merged: React.CSSProperties = { ...(style as React.CSSProperties) }
  if (numberOfLines && numberOfLines > 0) {
    merged.display = '-webkit-box'
    ;(merged as any).WebkitLineClamp = numberOfLines
    ;(merged as any).WebkitBoxOrient = 'vertical'
    merged.overflow = 'hidden'
  }
  return <span data-testid={testID} style={merged} {...rest}>{children}</span>
}

export type PressableProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  onLongPress?: () => void; delayLongPress?: number; testID?: string; style?: React.CSSProperties | any
}
export const Pressable: React.FC<PressableProps> = ({ children, onClick, onLongPress, delayLongPress = 300, style, testID, ...rest }) => {
  const timeout = React.useRef<number | null>(null)
  const handleMouseDown = () => {
    if (!onLongPress) return
    timeout.current = window.setTimeout(() => { onLongPress?.() }, delayLongPress)
  }
  const handleMouseUp = () => {
    if (timeout.current) { clearTimeout(timeout.current); timeout.current = null }
  }
  return (
    <button data-testid={testID} style={style as React.CSSProperties} onMouseDown={handleMouseDown} onMouseUp={handleMouseUp} onClick={onClick as any} {...rest}>
      {children}
    </button>
  )
}

export default { View, Text, Pressable }

