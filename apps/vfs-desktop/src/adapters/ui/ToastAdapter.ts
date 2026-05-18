/**
 * Toast Adapter
 *
 * Implements IToastService using toast hook
 *
 * Note: This adapter needs to be created with a toast instance
 * from the useToast hook. For dependency injection, we'll use
 * a factory pattern.
 */
import { IToastService, ToastOptions } from '../../ports/ui/IToastService';
import { ToastType } from '../../domain/enums/ToastType';

export interface ToastInstance {
  show: (
    message: string,
    options?: { duration?: number; type?: ToastType },
  ) => void;
  showActionToast: (message: string, shortcut?: string) => void;
}

export class ToastAdapter implements IToastService {
  constructor(private readonly toast: ToastInstance) {}

  show(options: ToastOptions): void {
    this.toast.show(options.message, {
      duration: options.duration,
      type: options.type,
    });
  }

  showActionToast(message: string, shortcut?: string): void {
    this.toast.showActionToast(message, shortcut);
  }
}
