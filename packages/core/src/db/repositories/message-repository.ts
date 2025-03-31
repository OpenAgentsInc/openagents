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
   * Create a new message
   */
  async createMessage(message: UIMessage & { threadId: string }): Promise<UIMessage> {
    if (!this.database) {
      throw new Error('Database not initialized');
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
   */
  async getMessagesByThreadId(threadId: string): Promise<UIMessage[]> {
    if (!this.database) {
      throw new Error('Database not initialized');
    }

    const messages = await this.database.messages
      .find()
      .where('threadId')
      .eq(threadId)
      .exec();

    return messages.map(message => {
      return storedMessageToUIMessage(message.toJSON() as StoredMessage);
    });
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
