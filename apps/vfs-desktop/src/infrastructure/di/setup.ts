/**
 * Dependency Injection Setup
 *
 * Wire up all dependencies
 */
import { Container } from './Container';
import {
  TauriStorageAdapter,
  TauriFileOperationsAdapter,
  TauriClipboardAdapter,
} from '../../adapters/tauri';
import { DialogAdapter } from '../../adapters/ui/DialogAdapter';
import { EventBusAdapter } from '../../adapters/events/EventBusAdapter';
import {
  CopyFilesUseCase,
  CutFilesUseCase,
  PasteFilesUseCase,
  DeleteFilesUseCase,
  RenameFileUseCase,
} from '../../use_cases/file_operations';
import {
  LoadFilesListUseCase,
  LoadSourcesListUseCase,
  NavigateToPathUseCase,
} from '../../use_cases/navigation';
import { IStorageService } from '../../ports/storage/IStorageService';
import { IFileOperations } from '../../ports/storage/IFileOperations';
import { IClipboardService } from '../../ports/storage/IClipboardService';
import { IDialogService } from '../../ports/ui/IDialogService';
import { IEventBus } from '../../ports/events/IEventBus';

export function setupContainer(): Container {
  const container = new Container();

  // Register adapters as singletons
  container.register('IStorageService', () => new TauriStorageAdapter(), true);
  container.register(
    'IFileOperations',
    () => new TauriFileOperationsAdapter(),
    true,
  );
  container.register(
    'IClipboardService',
    () => new TauriClipboardAdapter(),
    true,
  );
  container.register('IDialogService', () => new DialogAdapter(), true);
  container.register('IEventBus', () => new EventBusAdapter(), true);

  // Register use cases (transient - can be created multiple times)
  container.register('CopyFilesUseCase', () => {
    return new CopyFilesUseCase(
      container.resolve<IClipboardService>('IClipboardService'),
      container.resolve<IEventBus>('IEventBus'),
    );
  });

  container.register('CutFilesUseCase', () => {
    return new CutFilesUseCase(
      container.resolve<IClipboardService>('IClipboardService'),
      container.resolve<IEventBus>('IEventBus'),
    );
  });

  container.register('PasteFilesUseCase', () => {
    return new PasteFilesUseCase(
      container.resolve<IClipboardService>('IClipboardService'),
      container.resolve<IEventBus>('IEventBus'),
    );
  });

  container.register('DeleteFilesUseCase', () => {
    return new DeleteFilesUseCase(
      container.resolve<IFileOperations>('IFileOperations'),
      container.resolve<IDialogService>('IDialogService'),
      container.resolve<IEventBus>('IEventBus'),
    );
  });

  container.register('RenameFileUseCase', () => {
    return new RenameFileUseCase(
      container.resolve<IFileOperations>('IFileOperations'),
      container.resolve<IEventBus>('IEventBus'),
    );
  });

  container.register('LoadFilesListUseCase', () => {
    return new LoadFilesListUseCase(
      container.resolve<IFileOperations>('IFileOperations'),
    );
  });

  container.register('LoadSourcesListUseCase', () => {
    return new LoadSourcesListUseCase(
      container.resolve<IStorageService>('IStorageService'),
    );
  });

  container.register('NavigateToPathUseCase', () => {
    return new NavigateToPathUseCase();
  });

  return container;
}

// Export singleton container instance
let containerInstance: Container | null = null;

export function getContainer(): Container {
  if (!containerInstance) {
    containerInstance = setupContainer();
  }
  return containerInstance;
}
