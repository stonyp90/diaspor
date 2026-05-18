/**
 * Storage Service - Abstraction layer for data persistence
 *
 * This service provides a unified interface for storing and retrieving data.
 * Currently uses local storage (sessionStorage/localStorage), but designed to
 * easily migrate to backend API calls when ready.
 *
 * Migration path:
 * - Replace localStorage calls with API calls
 * - Keep the same interface
 * - Data models are already JSON-serializable and backend-ready
 */

export interface StorageAdapter {
  get<T>(key: string): Promise<T | null>;
  set<T>(key: string, value: T): Promise<void>;
  remove(key: string): Promise<void>;
  clear(): Promise<void>;
  getAllKeys(): Promise<string[]>;
}

/**
 * LocalStorage adapter (current implementation)
 */
class LocalStorageAdapter implements StorageAdapter {
  async get<T>(key: string): Promise<T | null> {
    try {
      const item = localStorage.getItem(key);
      if (!item) return null;
      return JSON.parse(item) as T;
    } catch (error) {
      console.error(`Failed to get ${key} from localStorage:`, error);
      return null;
    }
  }

  async set<T>(key: string, value: T): Promise<void> {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch (error) {
      console.error(`Failed to set ${key} in localStorage:`, error);
      throw error;
    }
  }

  async remove(key: string): Promise<void> {
    try {
      localStorage.removeItem(key);
    } catch (error) {
      console.error(`Failed to remove ${key} from localStorage:`, error);
      throw error;
    }
  }

  async clear(): Promise<void> {
    try {
      localStorage.clear();
    } catch (error) {
      console.error('Failed to clear localStorage:', error);
      throw error;
    }
  }

  async getAllKeys(): Promise<string[]> {
    try {
      return Object.keys(localStorage);
    } catch (error) {
      console.error('Failed to get all keys from localStorage:', error);
      return [];
    }
  }
}

/**
 * SessionStorage adapter
 */
class SessionStorageAdapter implements StorageAdapter {
  async get<T>(key: string): Promise<T | null> {
    try {
      const item = sessionStorage.getItem(key);
      if (!item) return null;
      return JSON.parse(item) as T;
    } catch (error) {
      console.error(`Failed to get ${key} from sessionStorage:`, error);
      return null;
    }
  }

  async set<T>(key: string, value: T): Promise<void> {
    try {
      sessionStorage.setItem(key, JSON.stringify(value));
    } catch (error) {
      console.error(`Failed to set ${key} in sessionStorage:`, error);
      throw error;
    }
  }

  async remove(key: string): Promise<void> {
    try {
      sessionStorage.removeItem(key);
    } catch (error) {
      console.error(`Failed to remove ${key} from sessionStorage:`, error);
      throw error;
    }
  }

  async clear(): Promise<void> {
    try {
      sessionStorage.clear();
    } catch (error) {
      console.error('Failed to clear sessionStorage:', error);
      throw error;
    }
  }

  async getAllKeys(): Promise<string[]> {
    try {
      return Object.keys(sessionStorage);
    } catch (error) {
      console.error('Failed to get all keys from sessionStorage:', error);
      return [];
    }
  }
}

/**
 * Backend API adapter (for future migration)
 * Uncomment and implement when backend is ready
 */
/*
class BackendApiAdapter implements StorageAdapter {
  private baseUrl: string;
  private authToken?: string;

  constructor(baseUrl: string, authToken?: string) {
    this.baseUrl = baseUrl;
    this.authToken = authToken;
  }

  async get<T>(key: string): Promise<T | null> {
    try {
      const response = await fetch(`${this.baseUrl}/storage/${encodeURIComponent(key)}`, {
        headers: {
          ...(this.authToken && { Authorization: `Bearer ${this.authToken}` }),
        },
      });
      if (!response.ok) {
        if (response.status === 404) return null;
        throw new Error(`Failed to get ${key}: ${response.statusText}`);
      }
      return await response.json() as T;
    } catch (error) {
      console.error(`Failed to get ${key} from backend:`, error);
      return null;
    }
  }

  async set<T>(key: string, value: T): Promise<void> {
    try {
      const response = await fetch(`${this.baseUrl}/storage/${encodeURIComponent(key)}`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          ...(this.authToken && { Authorization: `Bearer ${this.authToken}` }),
        },
        body: JSON.stringify(value),
      });
      if (!response.ok) {
        throw new Error(`Failed to set ${key}: ${response.statusText}`);
      }
    } catch (error) {
      console.error(`Failed to set ${key} in backend:`, error);
      throw error;
    }
  }

  async remove(key: string): Promise<void> {
    try {
      const response = await fetch(`${this.baseUrl}/storage/${encodeURIComponent(key)}`, {
        method: 'DELETE',
        headers: {
          ...(this.authToken && { Authorization: `Bearer ${this.authToken}` }),
        },
      });
      if (!response.ok && response.status !== 404) {
        throw new Error(`Failed to remove ${key}: ${response.statusText}`);
      }
    } catch (error) {
      console.error(`Failed to remove ${key} from backend:`, error);
      throw error;
    }
  }

  async clear(): Promise<void> {
    try {
      const response = await fetch(`${this.baseUrl}/storage`, {
        method: 'DELETE',
        headers: {
          ...(this.authToken && { Authorization: `Bearer ${this.authToken}` }),
        },
      });
      if (!response.ok) {
        throw new Error(`Failed to clear storage: ${response.statusText}`);
      }
    } catch (error) {
      console.error('Failed to clear backend storage:', error);
      throw error;
    }
  }

  async getAllKeys(): Promise<string[]> {
    try {
      const response = await fetch(`${this.baseUrl}/storage/keys`, {
        headers: {
          ...(this.authToken && { Authorization: `Bearer ${this.authToken}` }),
        },
      });
      if (!response.ok) {
        throw new Error(`Failed to get keys: ${response.statusText}`);
      }
      return await response.json() as string[];
    } catch (error) {
      console.error('Failed to get all keys from backend:', error);
      return [];
    }
  }
}
*/

/**
 * Storage Service
 *
 * Provides a unified interface for data persistence.
 * Currently uses localStorage for persistent data and sessionStorage for session data.
 *
 * To migrate to backend:
 * 1. Uncomment BackendApiAdapter above
 * 2. Replace adapter instances with BackendApiAdapter
 * 3. Update StorageService constructor to accept backend URL and auth token
 */
export class StorageService {
  private persistentAdapter: StorageAdapter;
  private sessionAdapter: StorageAdapter;

  constructor(
    persistentAdapter?: StorageAdapter,
    sessionAdapter?: StorageAdapter,
  ) {
    // Use provided adapters or default to localStorage/sessionStorage
    this.persistentAdapter = persistentAdapter || new LocalStorageAdapter();
    this.sessionAdapter = sessionAdapter || new SessionStorageAdapter();
  }

  /**
   * Get value from persistent storage
   */
  async get<T>(key: string): Promise<T | null> {
    return this.persistentAdapter.get<T>(key);
  }

  /**
   * Set value in persistent storage
   */
  async set<T>(key: string, value: T): Promise<void> {
    return this.persistentAdapter.set(key, value);
  }

  /**
   * Remove value from persistent storage
   */
  async remove(key: string): Promise<void> {
    return this.persistentAdapter.remove(key);
  }

  /**
   * Get value from session storage
   */
  async getSession<T>(key: string): Promise<T | null> {
    return this.sessionAdapter.get<T>(key);
  }

  /**
   * Set value in session storage
   */
  async setSession<T>(key: string, value: T): Promise<void> {
    return this.sessionAdapter.set(key, value);
  }

  /**
   * Remove value from session storage
   */
  async removeSession(key: string): Promise<void> {
    return this.sessionAdapter.remove(key);
  }

  /**
   * Clear all persistent storage
   */
  async clear(): Promise<void> {
    return this.persistentAdapter.clear();
  }

  /**
   * Clear all session storage
   */
  async clearSession(): Promise<void> {
    return this.sessionAdapter.clear();
  }

  /**
   * Get all keys from persistent storage
   */
  async getAllKeys(): Promise<string[]> {
    return this.persistentAdapter.getAllKeys();
  }

  /**
   * Get all keys from session storage
   */
  async getAllSessionKeys(): Promise<string[]> {
    return this.sessionAdapter.getAllKeys();
  }
}

// Export singleton instance
export const storageService = new StorageService();
