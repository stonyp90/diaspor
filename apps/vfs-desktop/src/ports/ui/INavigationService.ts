/**
 * Navigation Service Port
 *
 * Interface for navigation operations
 */
export interface INavigationService {
  /**
   * Navigate to a path
   */
  navigateToPath(sourceId: string, path: string): Promise<void>;

  /**
   * Navigate up one level
   */
  navigateUp(sourceId: string, currentPath: string): Promise<string>;

  /**
   * Navigate back in history
   */
  navigateBack(): Promise<void>;

  /**
   * Navigate forward in history
   */
  navigateForward(): Promise<void>;
}
