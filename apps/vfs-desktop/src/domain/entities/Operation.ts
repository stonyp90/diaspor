/**
 * Operation Entity
 *
 * Represents a tracked operation (upload, download, paste, etc.)
 */
import { OperationType } from '../enums/OperationType';
import { OperationStatus } from '../enums/OperationStatus';

// Re-export enums for convenience
export { OperationType, OperationStatus };

export interface Operation {
  id: string;
  type: OperationType;
  status: OperationStatus;
  sourceId?: string;
  sourcePath?: string;
  destId?: string;
  destPath?: string;
  progress: number; // 0.0 to 1.0
  totalBytes?: number;
  currentBytes?: number;
  speedBytesPerSec?: number;
  estimatedTimeRemaining?: number;
  error?: string;
  startedAt: string;
  completedAt?: string;
  files?: string[]; // List of file paths involved
}

/**
 * Operation Domain Methods
 */
export class OperationEntity {
  constructor(private readonly operation: Operation) {}

  isCompleted(): boolean {
    return this.operation.status === OperationStatus.Completed;
  }

  isFailed(): boolean {
    return this.operation.status === OperationStatus.Failed;
  }

  isInProgress(): boolean {
    return this.operation.status === OperationStatus.InProgress;
  }

  canBeCancelled(): boolean {
    return (
      this.operation.status === OperationStatus.Pending ||
      this.operation.status === OperationStatus.InProgress
    );
  }

  getProgressPercentage(): number {
    return Math.round(this.operation.progress * 100);
  }

  toPlainObject(): Operation {
    return { ...this.operation };
  }
}
