import React, { useState } from 'react'
import { StyleSheet, Platform } from 'react-native'
import { ArrowUp } from 'lucide-react'
import { View, TextInput, Pressable } from '@openagents/core'
import { react19 } from '@openagents/core'

// Make icons React 19 compatible
interface IconProps {
  size?: number;
  color?: string;
  [key: string]: any;
}
const ArrowUpIcon = react19.icon<IconProps>(ArrowUp)

interface MessageInputProps {
  maxRows?: number
  onSubmit?: (message: string) => void
}

export const MessageInput = ({ maxRows = 8, onSubmit }: MessageInputProps) => {
  const [value, setValue] = useState('')
  const [height, setHeight] = useState(0)

  const lineHeight = 20 // Approximate line height based on fontSize and padding
  const maxHeight = lineHeight * maxRows

  const handleSubmit = () => {
    if (!value.trim()) return
    onSubmit?.(value)
    setValue('')
    setHeight(0)
  }

  // More complete event type for different platforms
  type KeyPressEvent = {
    nativeEvent: { key: string; shiftKey?: boolean };
    preventDefault?: () => void;
  };

  const handleKeyPress = (e: KeyPressEvent) => {
    // Check if it's the Enter key
    if (e.nativeEvent.key === 'Enter') {
      // If shift is held, allow the newline
      if (e.nativeEvent.shiftKey) {
        return
      }
      // Prevent default newline behavior and submit
      if (e.preventDefault) {
        e.preventDefault()
      }
      handleSubmit()
    }
  }

  return (
    <View style={styles.inputWrapper}>
      <View style={styles.inputBorder} />
      <View style={styles.inputContainer}>
        <View style={styles.inputRow}>
          <TextInput
            autoFocus
            multiline
            value={value}
            onChangeText={setValue}
            onKeyPress={handleKeyPress}
            style={[
              styles.input,
              {
                height: Math.min(Math.max(lineHeight, height), maxHeight),
              },
            ]}
            onContentSizeChange={(e: { nativeEvent: { contentSize: { height: number } } }) => {
              setHeight(e.nativeEvent.contentSize.height)
            }}
            placeholder="Message Coder"
            placeholderTextColor="#666"
            numberOfLines={1}
            maxLength={2000}
          />
          <Pressable
            onPress={handleSubmit}
            style={[
              styles.submitButton,
              !value.trim() && styles.submitButtonDisabled
            ]}
          >
            <ArrowUpIcon size={20} color={value.trim() ? '#fff' : '#666'} />
          </Pressable>
        </View>
      </View>
    </View>
  )
}

const styles = StyleSheet.create({
  inputWrapper: {
    position: 'absolute',
    bottom: 0,
    left: 0,
    right: 0,
    backgroundColor: '#000',
  },
  inputBorder: {
    height: 1,
    backgroundColor: '#333',
    width: '100%',
  },
  inputContainer: {
    paddingVertical: 12,
    paddingHorizontal: '5%',
  },
  inputRow: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    position: 'relative',
  },
  input: {
    flex: 1,
    backgroundColor: 'transparent',
    borderWidth: 1,
    borderColor: '#fff',
    borderRadius: 0,
    padding: 12,
    paddingRight: 40, // Make room for the submit button
    color: '#fff',
    fontSize: 14,
    fontFamily: "Berkeley Mono",
    minHeight: 44, // Initial height for one line
    ...Platform.select({
      web: {
        // @ts-ignore - web-only style
        outlineStyle: 'none',
        // @ts-ignore - web-only style
        outlineWidth: 0,
      }
    })
  },
  submitButton: {
    position: 'absolute',
    right: 8,
    bottom: 8,
    width: 28,
    height: 28,
    borderRadius: 14,
    backgroundColor: 'transparent',
    alignItems: 'center',
    justifyContent: 'center',
  },
  submitButtonDisabled: {
    opacity: 0.5,
  },
})
