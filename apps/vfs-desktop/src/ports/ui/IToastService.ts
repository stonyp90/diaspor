/**
 * Toast Service Port
 *
 * Interface for toast notifications
 */
import { ToastType } from '../../domain/enums/ToastType';

export interface ToastOptions {
  message: string;
  duration?: number;
  type?: ToastType;
}

export interface IToastService {
  /**
   * Show a toast notification
   */
  show(options: ToastOptions): void;

  /**
   * Show an action toast (with shortcut hint)
   */
  showActionToast(message: string, shortcut?: string): void;
}
