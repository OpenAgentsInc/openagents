import React from 'react'
import { Animated, Easing, View, type LayoutChangeEvent, type StyleProp, type ViewStyle } from 'react-native'

export type CollapsibleProps = {
  open: boolean
  children?: React.ReactNode
  duration?: number
  style?: StyleProp<ViewStyle>
  contentStyle?: StyleProp<ViewStyle>
}

// Lightweight RN collapsible: animates height between 0 and measured content height.
// No external theming or dependencies required.
export function Collapsible({ open, children, duration = 200, style, contentStyle }: CollapsibleProps) {
  const [contentHeight, setContentHeight] = React.useState(0)
  const height = React.useRef(new Animated.Value(0)).current
  const hasMeasuredRef = React.useRef(false)

  const onLayout = React.useCallback((e: LayoutChangeEvent) => {
    const h = Math.max(0, Math.round(e.nativeEvent.layout.height))
    if (h !== contentHeight) {
      setContentHeight(h)
      if (!hasMeasuredRef.current) {
        // Initialize to the correct starting height without animating on first paint
        hasMeasuredRef.current = true
        height.setValue(open ? h : 0)
      }
    }
  }, [contentHeight, height, open])

  React.useEffect(() => {
    const target = open ? contentHeight : 0
    Animated.timing(height, {
      toValue: target,
      duration,
      easing: Easing.out(Easing.cubic),
      useNativeDriver: false, // height is a layout property
    }).start()
  }, [open, contentHeight, duration, height])

  return (
    <Animated.View style={[{ overflow: 'hidden', height }, style]}>
      <View onLayout={onLayout} style={contentStyle}>
        {children}
      </View>
    </Animated.View>
  )
}

export default Collapsible

