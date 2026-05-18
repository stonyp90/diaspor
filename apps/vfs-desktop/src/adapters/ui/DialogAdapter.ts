/**
 * Dialog Adapter
 *
 * Implements IDialogService using DialogService
 */
import {
  IDialogService,
  DialogOptions,
  ConfirmOptions,
} from '../../ports/ui/IDialogService';
import { DialogService } from '../../services/dialog';
import { DialogType } from '../../domain/enums/DialogType';

export class DialogAdapter implements IDialogService {
  async showMessage(options: DialogOptions): Promise<void> {
    await DialogService.message(options);
  }

  async showError(message: string, title?: string): Promise<void> {
    await DialogService.error(message, title);
  }

  async showWarning(message: string, title?: string): Promise<void> {
    await DialogService.warning(message, title);
  }

  async showInfo(message: string, title?: string): Promise<void> {
    await DialogService.info(message, title);
  }

  private mapDialogType(type?: DialogType): 'info' | 'warning' | 'error' {
    switch (type) {
      case DialogType.Error:
        return 'error';
      case DialogType.Warning:
        return 'warning';
      case DialogType.Info:
      default:
        return 'info';
    }
  }

  async showConfirm(options: ConfirmOptions): Promise<boolean> {
    return await DialogService.confirm(options);
  }

  async showOpenDialog(options?: {
    title?: string;
    directory?: boolean;
    multiple?: boolean;
    filters?: { name: string; extensions: string[] }[];
  }): Promise<string[] | null> {
    return await DialogService.open(options);
  }

  async showSaveDialog(options?: {
    title?: string;
    defaultPath?: string;
    filters?: { name: string; extensions: string[] }[];
  }): Promise<string | null> {
    return await DialogService.save(options);
  }
}
