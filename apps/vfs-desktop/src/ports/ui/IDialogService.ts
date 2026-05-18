/**
 * Dialog Service Port
 *
 * Interface for dialog operations
 */
import { DialogType } from '../../domain/enums/DialogType';

export interface DialogOptions {
  title?: string;
  message: string;
  type?: DialogType;
  okLabel?: string;
  cancelLabel?: string;
}

export interface ConfirmOptions extends DialogOptions {
  cancelLabel?: string;
}

export interface IDialogService {
  /**
   * Show a message dialog
   */
  showMessage(options: DialogOptions): Promise<void>;

  /**
   * Show an error dialog
   */
  showError(message: string, title?: string): Promise<void>;

  /**
   * Show a warning dialog
   */
  showWarning(message: string, title?: string): Promise<void>;

  /**
   * Show an info dialog
   */
  showInfo(message: string, title?: string): Promise<void>;

  /**
   * Show a confirmation dialog
   */
  showConfirm(options: ConfirmOptions): Promise<boolean>;

  /**
   * Show a file open dialog
   */
  showOpenDialog(options?: {
    title?: string;
    directory?: boolean;
    multiple?: boolean;
    filters?: { name: string; extensions: string[] }[];
  }): Promise<string[] | null>;

  /**
   * Show a save dialog
   */
  showSaveDialog(options?: {
    title?: string;
    defaultPath?: string;
    filters?: { name: string; extensions: string[] }[];
  }): Promise<string | null>;
}
