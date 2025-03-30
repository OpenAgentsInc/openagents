import React, { useState } from 'react'
import { StyleSheet } from 'react-native'
import { ToolInvocation } from './types'
import { ChevronIcon } from './icons/ChevronIcon'
import { View, Text, Pressable } from '@openagents/core'

interface ToolCallProps {
  toolInvocation: ToolInvocation
}

const iconStyle = {
  marginRight: 8
}

export const ToolCall = ({ toolInvocation }: ToolCallProps) => {
  const [isExpanded, setIsExpanded] = useState(false)
  const { toolName, toolCallId, state } = toolInvocation

  // Get dynamic data that varies between tool calls
  const dynamicData = {
    ...toolInvocation.args,
    ...(state === 'result' && { result: toolInvocation.result }),
  }

  return (
    <View style={styles.container}>
      <Pressable
        onPress={() => setIsExpanded(!isExpanded)}
        style={styles.header}
      >
        <View style={styles.headerLeft}>
          <ChevronIcon
            direction={isExpanded ? 'down' : 'right'}
            size={16}
            style={iconStyle}
          />
          <Text style={styles.toolName}>{toolName}</Text>
        </View>
        <Text style={styles.state}>{state}</Text>
      </Pressable>
      {isExpanded && (
        <View style={styles.details}>
          <Text style={styles.id}>ID: {toolCallId}</Text>
          <Text style={styles.data}>{JSON.stringify(dynamicData, null, 2)}</Text>
        </View>
      )}
    </View>
  )
}

const styles = StyleSheet.create({
  container: {
    backgroundColor: 'rgba(255, 255, 255, 0.1)',
    borderRadius: 4,
    padding: 12,
    marginTop: 8,
  },
  header: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
  },
  headerLeft: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  toolName: {
    color: '#fff',
    fontFamily: 'Berkeley Mono',
    fontSize: 14,
    fontWeight: 'bold',
    userSelect: 'none',
  },
  state: {
    color: '#999',
    fontFamily: 'Berkeley Mono',
    fontSize: 12,
    textTransform: 'uppercase',
    userSelect: 'none',
  },
  details: {
    marginTop: 8,
    paddingTop: 8,
    borderTopWidth: 1,
    borderTopColor: 'rgba(255, 255, 255, 0.1)',
  },
  id: {
    color: '#999',
    fontFamily: 'Berkeley Mono',
    fontSize: 12,
    marginBottom: 8,
  },
  data: {
    color: '#fff',
    fontFamily: 'Berkeley Mono',
    fontSize: 12,
    lineHeight: 18,
  },
})
