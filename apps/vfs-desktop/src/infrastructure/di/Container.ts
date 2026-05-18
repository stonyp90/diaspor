/**
 * Dependency Injection Container
 *
 * Simple DI container for managing service dependencies
 */
export class Container {
  private services = new Map<string, () => unknown>();
  private singletons = new Map<string, unknown>();

  /**
   * Register a service factory
   */
  register<T>(key: string, factory: () => T, singleton = false): void {
    if (singleton) {
      // For singletons, store the factory and create on first resolve
      this.services.set(key, factory);
      this.singletons.set(key, null);
    } else {
      // For transient services, store the factory
      this.services.set(key, factory);
    }
  }

  /**
   * Register a singleton instance directly
   */
  registerInstance<T>(key: string, instance: T): void {
    this.singletons.set(key, instance);
  }

  /**
   * Resolve a service
   */
  resolve<T>(key: string): T {
    // Check if it's a registered singleton instance
    if (this.singletons.has(key)) {
      const instance = this.singletons.get(key);
      if (instance !== null) {
        return instance as T;
      }
      // Singleton not created yet, create it
      const factory = this.services.get(key);
      if (factory) {
        const newInstance = factory();
        this.singletons.set(key, newInstance);
        return newInstance as T;
      }
    }

    // Check if it's a transient service
    const factory = this.services.get(key);
    if (!factory) {
      throw new Error(`Service ${key} not found`);
    }

    return factory() as T;
  }

  /**
   * Check if a service is registered
   */
  has(key: string): boolean {
    return this.services.has(key) || this.singletons.has(key);
  }

  /**
   * Clear all registered services
   */
  clear(): void {
    this.services.clear();
    this.singletons.clear();
  }
}
