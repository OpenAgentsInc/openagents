import { v4 as uuidv4 } from 'uuid';
import { getDatabase } from '../database';
import {
  Database,
  StoredMessage,
  MessageDocument,
  storedMessageToUIMessage,
  uiMessageToStoredMessage
} from '../types';
import { UIMessage } from '../../chat/types';
import { DeepReadonlyObject } from 'rxdb';

/**
 * Repository for message operations
 */
export class MessageRepository {
  private database: Database | null = null;

  /**
   * Initialize the repository with a database connection
   */
  async initialize(database: Database) {
    this.database = database;
  }

  /**
   * Create a new message with precise timestamp handling
   */
  async createMessage(message: UIMessage & { threadId: string }): Promise<UIMessage> {
    if (!this.database) {
      throw new Error('Database not initialized');
    }

    // Ensure the message has a unique timestamp if not already set
    if (!message.createdAt) {
      message.createdAt = new Date();
    }

    // Get the latest message timestamp to ensure proper ordering
    try {
      const latestMessage = await this.database.messages
        .find()
        .where('threadId')
        .eq(message.threadId)
        .sort({ createdAt: 'desc' })
        .limit(1)
        .exec();

      if (latestMessage.length > 0) {
        const latestTimestamp = latestMessage[0].createdAt;

        // ALWAYS increment timestamp by at least 500ms to ensure visible differences
        // This enforces a minimum gap between messages
        const minimumTimestampIncrement = 500; // 500ms gap between messages

        // Calculate the appropriate timestamp - either current time or forced increment
        const currentTime = message.createdAt.getTime();
        // Add role factor to ensure user/assistant messages can't have the same timestamp
        const roleFactor = message.role === 'user' ? 0 : 250; // 250ms offset for assistant messages

        const nextValidTimestamp = Math.max(
          currentTime + roleFactor,
          latestTimestamp + minimumTimestampIncrement + roleFactor
        );

        // Always set a new timestamp with the calculated value
        message.createdAt = new Date(nextValidTimestamp);
        // console.log(`Set timestamp for message ${message.id} to: ${message.createdAt}`);
      }
    } catch (err) {
      console.error('Error checking latest timestamp:', err);
      // Continue with creation even if timestamp check fails
    }

    const storedMessage = uiMessageToStoredMessage(message, message.threadId);

    try {
      // Try to insert the message
      const doc = await this.database.messages.insert(storedMessage);
      return storedMessageToUIMessage(doc.toJSON() as StoredMessage);
    } catch (error: any) {
      // If we get a conflict error, the document already exists
      if (error.code === 'CONFLICT') {
        console.log(`Message with ID ${message.id} already exists, skipping insert`);
        // Get the existing message
        const existingMessage = await this.database.messages.findOne(message.id).exec();
        if (existingMessage) {
          return storedMessageToUIMessage(existingMessage.toJSON() as StoredMessage);
        } else {
          throw error; // Shouldn't happen but just in case
        }
      } else {
        // For any other error, just rethrow
        throw error;
      }
    }
  }

  /**
   * Get a message by ID
   */
  async getMessageById(id: string): Promise<UIMessage | null> {
    if (!this.database) {
      throw new Error('Database not initialized');
    }

    const message = await this.database.messages.findOne(id).exec();
    if (!message) {
      return null;
    }

    return storedMessageToUIMessage(message.toJSON() as StoredMessage);
  }

  /**
   * Get all messages for a thread, sorted by creation time
   * Ensures proper timestamps for UI rendering
   */
  async getMessagesByThreadId(threadId: string): Promise<UIMessage[]> {
    if (!this.database) {
      throw new Error('Database not initialized');
    }

    const messages = await this.database.messages
      .find()
      .where('threadId')
      .eq(threadId)
      .sort({ createdAt: 'asc' })
      .exec();

    // First convert all messages to UIMessages
    const uiMessages = messages.map(message =>
      storedMessageToUIMessage(message.toJSON() as StoredMessage)
    );

    // Now fix any timestamp issues to ensure proper ordering when rehydrating from DB
    // If more than one message has the exact same timestamp, fix them
    const fixedMessages = this.fixMessageTimestamps(uiMessages);

    return fixedMessages;
  }

  /**
   * Fixes timestamps for messages loaded from database
   * Ensures chronological separation between messages
   */
  fixMessageTimestamps(messages: UIMessage[]): UIMessage[] {
    // If no messages or just one message, no need to fix
    if (messages.length <= 1) {
      return messages;
    }

    // Group messages by timestamp
    const messagesByTimestamp: Record<number, UIMessage[]> = {};

    // First identify if we have any timestamp collisions
    let hasCollisions = false;

    messages.forEach(message => {
      const timestamp = message.createdAt?.getTime() || 0;
      if (!messagesByTimestamp[timestamp]) {
        messagesByTimestamp[timestamp] = [];
      }
      messagesByTimestamp[timestamp].push(message);

      // If we have more than one message with the same timestamp, we have collisions
      if (messagesByTimestamp[timestamp].length > 1) {
        hasCollisions = true;
      }
    });

    // If no collisions, return the original messages
    if (!hasCollisions) {
      return messages;
    }

    console.log("Fixing timestamp collisions for rehydrated messages");

    // Sort messages by timestamp (ascending)
    const sortedMessages = [...messages].sort((a, b) => {
      const timeA = a.createdAt?.getTime() || 0;
      const timeB = b.createdAt?.getTime() || 0;
      return timeA - timeB;
    });

    // Now fix collisions by incrementing timestamps
    // Start with the earliest timestamp
    let lastTimestamp = sortedMessages[0].createdAt?.getTime() || 0;
    const fixedMessages = sortedMessages.map((message, index) => {
      if (index === 0) return message; // Keep the first message's timestamp

      const currentTimestamp = message.createdAt?.getTime() || 0;
      const timeGap = 500; // 500ms between messages for clear separation

      // If this message has the same timestamp as previous, or less than minimum gap
      if (currentTimestamp <= lastTimestamp + timeGap) {
        // Create new timestamp with proper gap
        const newTimestamp = lastTimestamp + timeGap;

        // Basic order - user messages come before assistant if same timestamp
        const roleFactor = message.role === 'user' ? 0 : 250;

        // Create a new date with the adjusted timestamp
        const newDate = new Date(newTimestamp + roleFactor);

        // Clone the message with the new timestamp
        const fixedMessage = {
          ...message,
          createdAt: newDate
        };

        // Update lastTimestamp to this new one
        lastTimestamp = newTimestamp + roleFactor;

        console.log(`Fixed timestamp for message ${message.id} from ${message.createdAt} to ${newDate}`);
        return fixedMessage;
      }

      // This message already has a good timestamp gap, keep it
      lastTimestamp = currentTimestamp;
      return message;
    });

    return fixedMessages;
  }

  /**
   * Update a message
   */
  async updateMessage(id: string, updates: Partial<UIMessage>): Promise<UIMessage | null> {
    if (!this.database) {
      throw new Error('Database not initialized');
    }

    const message = await this.database.messages.findOne(id).exec();
    if (!message) {
      return null;
    }

    // Prepare updates in stored format
    const storedUpdates: Partial<StoredMessage> = {
      ...(updates.content !== undefined && { content: updates.content }),
      ...(updates.parts !== undefined && { parts: updates.parts }),
      ...(updates.experimental_attachments !== undefined && {
        attachments: updates.experimental_attachments
      })
    };

    await message.update({
      $set: storedUpdates
    });

    return storedMessageToUIMessage(message.toJSON() as StoredMessage);
  }

  /**
   * Delete a message
   */
  async deleteMessage(id: string): Promise<boolean> {
    if (!this.database) {
      throw new Error('Database not initialized');
    }

    const message = await this.database.messages.findOne(id).exec();
    if (!message) {
      return false;
    }

    await message.remove();
    return true;
  }

  /**
   * Delete all messages for a thread
   */
  async deleteMessagesByThreadId(threadId: string): Promise<number> {
    if (!this.database) {
      throw new Error('Database not initialized');
    }

    const messages = await this.database.messages
      .find({ selector: { threadId } })
      .exec();

    for (const message of messages) {
      await message.remove();
    }

    return messages.length;
  }

  /**
   * Bulk insert messages
   */
  async bulkInsertMessages(messages: Array<UIMessage & { threadId: string }>): Promise<UIMessage[]> {
    if (!this.database) {
      throw new Error('Database not initialized');
    }

    const storedMessages = messages.map(message =>
      uiMessageToStoredMessage(message, message.threadId)
    );

    // Insert each message
    for (const message of storedMessages) {
      await this.database.messages.insert(message);
    }

    return storedMessages.map(storedMessageToUIMessage);
  }

  /**
   * Get reactive query for messages in a thread
   */
  async getMessagesQuery(threadId: string) {
    if (!this.database) {
      throw new Error('Database not initialized');
    }

    return this.database.messages
      .find({ selector: { threadId } })
      .sort({ createdAt: 'asc' });
  }
}

// Singleton instance
export const messageRepository = new MessageRepository();
