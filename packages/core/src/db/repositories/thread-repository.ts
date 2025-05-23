import { v4 as uuidv4 } from 'uuid';
import { getDatabase } from '../database';
import type { Thread, ThreadDocument, Database } from '../types';

/**
 * Repository for thread operations
 */
export class ThreadRepository {
  private db: Database | null = null;

  /**
   * Initialize the repository with a database connection
   * Can optionally accept a database instance to avoid multiple initializations
   */
  async initialize(database?: Database): Promise<void> {
    if (!this.db) {
      if (database) {
        this.db = database;
      } else {
        this.db = await getDatabase();
      }
    }
  }

  /**
   * Create a new thread
   */
  async createThread(threadData: Partial<Thread>): Promise<Thread> {
    try {
      await this.initialize();

      const currentTime = Date.now();
      const thread: Thread = {
        id: threadData.id || uuidv4(),
        title: threadData.title || 'New Chat',
        createdAt: threadData.createdAt || currentTime,
        updatedAt: threadData.updatedAt || currentTime,
        modelId: threadData.modelId || '',  // Use empty string instead of null/undefined
        systemPrompt: threadData.systemPrompt || '',  // Use empty string instead of null/undefined
        metadata: threadData.metadata || {}
      };

      try {
        // console.log('Creating thread with data:', thread);
        await this.db!.threads.insert(thread);
        return thread;
      } catch (error) {
        // If insert fails but we have an ID, try to fetch existing thread
        if (thread.id) {
          const existingThread = await this.db!.threads.findOne(thread.id).exec();
          if (existingThread) {
            return existingThread.toJSON();
          }
        }
        throw error;
      }
    } catch (error) {
      console.error('Error creating thread:', error);
      // If database operation fails, still return the in-memory thread
      // so the application can continue to function
      return threadData as Thread;
    }
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
   * Get all threads, sorted by creation time (newest first)
   */
  async getAllThreads(): Promise<Thread[]> {
    try {
      await this.initialize();

      const threads = await this.db!.threads
        .find()
        .sort({ createdAt: 'desc' })
        .exec();

      return threads.map(thread => thread.toJSON());
    } catch (error) {
      console.error('Error fetching all threads:', error);
      // Return empty array as fallback so UI doesn't break
      return [];
    }
  }

  /**
   * Update a thread with retry mechanism for conflict resolution
   */
  async updateThread(id: string, updates: Partial<Thread>, maxRetries = 3): Promise<Thread | null> {
    await this.initialize();

    // Always update the updatedAt timestamp
    const updatedThread = {
      ...updates,
      updatedAt: Date.now()
    };

    let retries = 0;

    while (retries < maxRetries) {
      try {
        // Get fresh version of the document
        const thread = await this.db!.threads.findOne(id).exec();
        if (!thread) {
          return null;
        }

        await thread.update({
          $set: updatedThread
        });

        return thread.toJSON();
      } catch (error: any) {
        // If it's a conflict error, wait briefly and retry
        if (error.code === 'CONFLICT' && retries < maxRetries - 1) {
          console.log(`Conflict detected on thread ${id}, retrying (${retries + 1}/${maxRetries})...`);
          retries++;
          // Add a small delay between retries
          await new Promise(resolve => setTimeout(resolve, 50 * retries));
        } else {
          // For other errors or if we've reached max retries, throw the error
          console.error(`Update thread error after ${retries} retries:`, error);
          throw error;
        }
      }
    }

    throw new Error(`Failed to update thread ${id} after ${maxRetries} retries`);
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
      .sort({ createdAt: 'desc' });
  }
}

// Singleton instance
export const threadRepository = new ThreadRepository();
