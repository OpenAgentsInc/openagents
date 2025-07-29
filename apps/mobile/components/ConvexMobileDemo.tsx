import React, { useState } from "react";
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  FlatList,
  StyleSheet,
  Platform,
} from "react-native";
import { useQuery, useMutation } from "convex/react";
import { api } from '@openagentsinc/convex';
import { DARK_THEME } from "../constants/colors";

export function ConvexMobileDemo() {
  const messages = useQuery(api.messages.getMessages) || [];
  const messageCount = useQuery(api.messages.getMessageCount) || 0;
  const addMessage = useMutation(api.messages.addMessage);
  
  const [newMessage, setNewMessage] = useState("");
  const [userName] = useState(() => `Mobile-${Math.floor(Math.random() * 1000)}`);

  const handleSubmit = async () => {
    if (!newMessage.trim()) return;
    
    await addMessage({
      body: newMessage.trim(),
      user: userName
    });
    setNewMessage("");
  };

  return (
    <View style={styles.container}>
      <Text style={styles.title}>Convex Mobile Demo</Text>
      
      <View style={styles.infoContainer}>
        <Text style={styles.infoText}>
          Connected as: <Text style={styles.userText}>{userName}</Text>
        </Text>
        <Text style={styles.infoText}>
          Total messages: <Text style={styles.countText}>{messageCount}</Text>
        </Text>
      </View>

      <View style={styles.messagesContainer}>
        <Text style={styles.messagesTitle}>Recent Messages:</Text>
        {messages.length === 0 ? (
          <Text style={styles.emptyText}>No messages yet...</Text>
        ) : (
          <FlatList
            data={messages.slice(-5)}
            keyExtractor={(item) => item._id}
            renderItem={({ item }) => (
              <View style={styles.messageItem}>
                <Text style={styles.messageUser}>{item.user}:</Text>
                <Text style={styles.messageBody}>{item.body}</Text>
                <Text style={styles.messageTime}>
                  {new Date(item.timestamp).toLocaleTimeString()}
                </Text>
              </View>
            )}
            style={styles.messagesList}
          />
        )}
      </View>

      <View style={styles.inputContainer}>
        <TextInput
          style={styles.input}
          value={newMessage}
          onChangeText={setNewMessage}
          placeholder="Type a message..."
          placeholderTextColor={DARK_THEME.textTertiary}
          multiline
        />
        <TouchableOpacity
          style={[styles.button, !newMessage.trim() && styles.buttonDisabled]}
          onPress={handleSubmit}
          disabled={!newMessage.trim()}
        >
          <Text style={[styles.buttonText, !newMessage.trim() && styles.buttonTextDisabled]}>
            Send
          </Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: DARK_THEME.background,
    borderRadius: 8,
    padding: 16,
    borderWidth: 1,
    borderColor: DARK_THEME.border,
  },
  title: {
    fontSize: 18,
    fontWeight: 'bold',
    color: DARK_THEME.text,
    marginBottom: 16,
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono', 
      default: 'monospace'
    })
  },
  infoContainer: {
    marginBottom: 16,
  },
  infoText: {
    fontSize: 12,
    color: DARK_THEME.textSecondary,
    marginBottom: 4,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    })
  },
  userText: {
    color: '#4ade80', // Keep green for user text
  },
  countText: {
    color: DARK_THEME.primary,
  },
  messagesContainer: {
    flex: 1,
    marginBottom: 16,
  },
  messagesTitle: {
    fontSize: 14,
    color: DARK_THEME.textSecondary,
    marginBottom: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    })
  },
  messagesList: {
    flex: 1,
    backgroundColor: DARK_THEME.backgroundSecondary,
    borderRadius: 4,
    padding: 8,
  },
  emptyText: {
    color: DARK_THEME.textTertiary,
    fontSize: 12,
    textAlign: 'center',
    padding: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    })
  },
  messageItem: {
    marginBottom: 8,
    paddingBottom: 8,
    borderBottomWidth: 1,
    borderBottomColor: DARK_THEME.border,
  },
  messageUser: {
    fontSize: 12,
    color: '#22d3ee', // Keep cyan for message user
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    })
  },
  messageBody: {
    fontSize: 12,
    color: DARK_THEME.text,
    marginTop: 2,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    })
  },
  messageTime: {
    fontSize: 10,
    color: DARK_THEME.textTertiary,
    marginTop: 2,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    })
  },
  inputContainer: {
    flexDirection: 'row',
    gap: 8,
  },
  input: {
    flex: 1,
    backgroundColor: DARK_THEME.backgroundSecondary,
    color: DARK_THEME.text,
    padding: 12,
    borderRadius: 4,
    borderWidth: 1,
    borderColor: DARK_THEME.border,
    fontSize: 12,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    })
  },
  button: {
    backgroundColor: DARK_THEME.primary,
    paddingHorizontal: 16,
    paddingVertical: 12,
    borderRadius: 4,
    justifyContent: 'center',
  },
  buttonDisabled: {
    backgroundColor: DARK_THEME.disabled,
  },
  buttonText: {
    color: DARK_THEME.text,
    fontSize: 12,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    })
  },
  buttonTextDisabled: {
    color: DARK_THEME.textTertiary,
  },
});