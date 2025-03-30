import { v4 as uuidv4 } from 'uuid';
import { getDatabase } from '../database';
import { Thread, ThreadDocument, Database } from '../types';

/**
 * Repository for thread operations
 */
export class ThreadRepository {
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
   * Create a new thread
   */
  async createThread(threadData: Partial<Thread>): Promise<Thread> {
    await this.initialize();
    
    const currentTime = Date.now();
    const thread: Thread = {
      id: threadData.id || uuidv4(),
      title: threadData.title || 'New Chat',
      createdAt: threadData.createdAt || currentTime,
      updatedAt: threadData.updatedAt || currentTime,
      modelId: threadData.modelId,
      systemPrompt: threadData.systemPrompt,
      metadata: threadData.metadata || {}
    };
    
    await this.db!.threads.insert(thread);
    return thread;
  }
  
  /**
   * Get a thread by ID
   */
  async getThreadById(id: string): Promise<Thread | null> {
    await this.initialize();
    
    const thread = await this.db!.threads.findOne(id).exec();
    return thread ? thread.toJSON() : null;
  }
  
  /**
   * Get all threads, sorted by updated time (newest first)
   */
  async getAllThreads(): Promise<Thread[]> {
    await this.initialize();
    
    const threads = await this.db!.threads
      .find()
      .sort({ updatedAt: 'desc' })
      .exec();
      
    return threads.map(thread => thread.toJSON());
  }
  
  /**
   * Update a thread
   */
  async updateThread(id: string, updates: Partial<Thread>): Promise<Thread | null> {
    await this.initialize();
    
    // Always update the updatedAt timestamp
    const updatedThread = {
      ...updates,
      updatedAt: Date.now()
    };
    
    const thread = await this.db!.threads.findOne(id).exec();
    if (!thread) {
      return null;
    }
    
    await thread.update({
      $set: updatedThread
    });
    
    return thread.toJSON();
  }
  
  /**
   * Delete a thread
   */
  async deleteThread(id: string): Promise<boolean> {
    await this.initialize();
    
    const thread = await this.db!.threads.findOne(id).exec();
    if (!thread) {
      return false;
    }
    
    await thread.remove();
    return true;
  }
  
  /**
   * Get reactive query for threads
   */
  async getThreadsQuery() {
    await this.initialize();
    
    return this.db!.threads
      .find()
      .sort({ updatedAt: 'desc' });
  }
}

// Singleton instance
export const threadRepository = new ThreadRepository();