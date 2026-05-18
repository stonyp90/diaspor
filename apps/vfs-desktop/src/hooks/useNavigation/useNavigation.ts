/**
 * Navigation Hook
 *
 * Custom hook for navigation operations using use cases
 */
import { useCallback } from 'react';
import { getContainer } from '../../infrastructure/di';
import {
  LoadFilesListUseCase,
  LoadSourcesListUseCase,
  NavigateToPathUseCase,
} from '../../use_cases/navigation';

export function useNavigation() {
  const container = getContainer();

  const loadFilesList = useCallback(
    async (sourceId: string, path: string) => {
      const useCase = container.resolve<LoadFilesListUseCase>(
        'LoadFilesListUseCase',
      );
      return await useCase.execute({ sourceId, path });
    },
    [container],
  );

  const loadSourcesList = useCallback(async () => {
    const useCase = container.resolve<LoadSourcesListUseCase>(
      'LoadSourcesListUseCase',
    );
    return await useCase.execute();
  }, [container]);

  const navigateToPath = useCallback(
    async (sourceId: string, path: string) => {
      const useCase = container.resolve<NavigateToPathUseCase>(
        'NavigateToPathUseCase',
      );
      return await useCase.execute({ sourceId, path });
    },
    [container],
  );

  return {
    loadFilesList,
    loadSourcesList,
    navigateToPath,
  };
}
