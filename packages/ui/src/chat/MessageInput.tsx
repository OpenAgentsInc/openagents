import { View, TextInput, StyleSheet } from 'react-native'
import { useState } from 'react'

interface MessageInputProps {
  maxRows?: number
}

export const MessageInput = ({ maxRows = 8 }: MessageInputProps) => {
  const [value, setValue] = useState('')
  const [height, setHeight] = useState(0)

  const lineHeight = 20 // Approximate line height based on fontSize and padding
  const maxHeight = lineHeight * maxRows

  return (
    <View style={styles.inputWrapper}>
      <View style={styles.inputBorder} />
      <View style={styles.inputContainer}>
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
  input: {
    backgroundColor: 'transparent',
    borderWidth: 1,
    borderColor: '#fff',
    borderRadius: 8,
    padding: 12,
    color: '#fff',
    fontSize: 14,
    fontFamily: "Berkeley Mono",
    minHeight: 44, // Initial height for one line
  },
})
