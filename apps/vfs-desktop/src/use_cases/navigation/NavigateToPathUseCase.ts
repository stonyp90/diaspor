/**
 * Navigate To Path Use Case
 *
 * Business logic for navigating to a specific path
 */

export interface NavigateToPathRequest {
  sourceId: string;
  path: string;
}

export interface NavigateToPathResponse {
  success: boolean;
  normalizedPath?: string;
  error?: string;
}

export class NavigateToPathUseCase {
  async execute(
    request: NavigateToPathRequest,
  ): Promise<NavigateToPathResponse> {
    try {
      if (!request.sourceId) {
        return { success: false, error: 'No source selected' };
      }

      // Normalize path
      let normalizedPath = request.path;
      if (!normalizedPath.startsWith('/')) {
        normalizedPath = '/' + normalizedPath;
      }
      if (!normalizedPath.endsWith('/') && normalizedPath !== '/') {
        // Check if it's a directory by attempting to create Path object
        // For now, assume directories end with /
        // This could be enhanced with actual file system checks
      }

      return { success: true, normalizedPath };
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      return { success: false, error: errorMessage };
    }
  }
}
