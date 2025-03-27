import { View, TextInput, StyleSheet, Pressable } from 'react-native'
import { useState } from 'react'
import { ArrowUp } from 'lucide-react'

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
            style={[
              styles.input,
              {
                height: Math.min(Math.max(lineHeight, height), maxHeight),
              },
            ]}
            onContentSizeChange={(e) => {
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
            <ArrowUp size={20} color={value.trim() ? '#fff' : '#666'} />
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
    borderRadius: 8,
    padding: 12,
    paddingRight: 40, // Make room for the submit button
    color: '#fff',
    fontSize: 14,
    fontFamily: "Berkeley Mono",
    minHeight: 44, // Initial height for one line
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
