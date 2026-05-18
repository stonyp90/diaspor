/**
 * File Operations Hook
 *
 * Custom hook for file operations using use cases
 */
import { useCallback } from 'react';
import { getContainer } from '../../infrastructure/di';
import {
  CopyFilesUseCase,
  CutFilesUseCase,
  PasteFilesUseCase,
  DeleteFilesUseCase,
  RenameFileUseCase,
} from '../../use_cases/file_operations';
import { IToastService } from '../../ports/ui/IToastService';
import { ToastType } from '../../domain/enums/ToastType';

export function useFileOperations(toast: IToastService) {
  const container = getContainer();

  const copy = useCallback(
    async (sourceId: string, paths: string[]) => {
      const useCase = container.resolve<CopyFilesUseCase>('CopyFilesUseCase');
      const result = await useCase.execute({ sourceId, paths });
      if (!result.success && result.error) {
        toast.show({ message: result.error, type: ToastType.Error });
      }
      return result;
    },
    [container, toast],
  );

  const cut = useCallback(
    async (sourceId: string, paths: string[]) => {
      const useCase = container.resolve<CutFilesUseCase>('CutFilesUseCase');
      const result = await useCase.execute({ sourceId, paths });
      if (!result.success && result.error) {
        toast.show({ message: result.error, type: ToastType.Error });
      }
      return result;
    },
    [container, toast],
  );

  const paste = useCallback(
    async (destSourceId: string, destPath: string) => {
      const useCase = container.resolve<PasteFilesUseCase>('PasteFilesUseCase');
      const result = await useCase.execute({ destSourceId, destPath });
      if (!result.success && result.error) {
        toast.show({ message: result.error, type: ToastType.Error });
      }
      return result;
    },
    [container, toast],
  );

  const deleteFiles = useCallback(
    async (sourceId: string, paths: string[], skipConfirmation = false) => {
      const useCase =
        container.resolve<DeleteFilesUseCase>('DeleteFilesUseCase');
      const result = await useCase.execute({
        sourceId,
        paths,
        skipConfirmation,
      });
      if (!result.success && result.error) {
        toast.show({ message: result.error, type: ToastType.Error });
      }
      return result;
    },
    [container, toast],
  );

  const renameFile = useCallback(
    async (sourceId: string, oldPath: string, newName: string) => {
      const useCase = container.resolve<RenameFileUseCase>('RenameFileUseCase');
      const result = await useCase.execute({ sourceId, oldPath, newName });
      if (!result.success && result.error) {
        toast.show({ message: result.error, type: ToastType.Error });
      }
      return result;
    },
    [container, toast],
  );

  return {
    copy,
    cut,
    paste,
    deleteFiles,
    renameFile,
  };
}
