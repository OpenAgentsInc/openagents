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

/**
 * Repository for message operations
 */
export class MessageRepository {
  private db: Database | null = null;
  
  /**
   * Initialize the repository with a database connection
   */
  async initialize(): Promise<void> {
    if (!this.db) {
      this.db = await getDatabase();
    }
  }
  
  /**
   * Create a new message
   */
  async createMessage(messageData: UIMessage & { threadId: string }): Promise<UIMessage> {
    await this.initialize();
    
    // Convert to StoredMessage format
    const storedMessage = uiMessageToStoredMessage(messageData, messageData.threadId);
    
    // Ensure ID exists
    if (!storedMessage.id) {
      storedMessage.id = uuidv4();
    }
    
    // Insert into database
    await this.db!.messages.insert(storedMessage);
    
    // Return as UIMessage
    return storedMessageToUIMessage(storedMessage);
  }
  
  /**
   * Get a message by ID
   */
  async getMessageById(id: string): Promise<UIMessage | null> {
    await this.initialize();
    
    const message = await this.db!.messages.findOne(id).exec();
    if (!message) {
      return null;
    }
    
    return storedMessageToUIMessage(message.toJSON());
  }
  
  /**
   * Get all messages for a thread, sorted by creation time
   */
  async getMessagesByThreadId(threadId: string): Promise<UIMessage[]> {
    await this.initialize();
    
    const messages = await this.db!.messages
      .find({ selector: { threadId } })
      .sort({ createdAt: 'asc' })
      .exec();
      
    return messages.map(message => storedMessageToUIMessage(message.toJSON()));
  }
  
  /**
   * Update a message
   */
  async updateMessage(id: string, updates: Partial<UIMessage>): Promise<UIMessage | null> {
    await this.initialize();
    
    const message = await this.db!.messages.findOne(id).exec();
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
    
    return storedMessageToUIMessage(message.toJSON());
  }
  
  /**
   * Delete a message
   */
  async deleteMessage(id: string): Promise<boolean> {
    await this.initialize();
    
    const message = await this.db!.messages.findOne(id).exec();
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
    await this.initialize();
    
    const messages = await this.db!.messages
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
    await this.initialize();
    
    const storedMessages = messages.map(message => 
      uiMessageToStoredMessage(message, message.threadId)
    );
    
    // Insert each message
    for (const message of storedMessages) {
      await this.db!.messages.insert(message);
    }
    
    return storedMessages.map(storedMessageToUIMessage);
  }
  
  /**
   * Get reactive query for messages in a thread
   */
  async getMessagesQuery(threadId: string) {
    await this.initialize();
    
    return this.db!.messages
      .find({ selector: { threadId } })
      .sort({ createdAt: 'asc' });
  }
}

// Singleton instance
export const messageRepository = new MessageRepository();