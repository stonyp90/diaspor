/**
 * Clipboard Content Entity
 *
 * Represents clipboard content for copy/cut operations
 */
export type ClipboardOperation = 'copy' | 'cut';

export interface ClipboardContent {
  operation: ClipboardOperation;
  sourceId: string;
  paths: string[];
  timestamp: string;
}

/**
 * Clipboard Content Domain Methods
 */
export class ClipboardContentEntity {
  constructor(private readonly content: ClipboardContent) {}

  isCutOperation(): boolean {
    return this.content.operation === 'cut';
  }

  isCopyOperation(): boolean {
    return this.content.operation === 'copy';
  }

  getFileCount(): number {
    return this.content.paths.length;
  }

  isEmpty(): boolean {
    return this.content.paths.length === 0;
  }

  toPlainObject(): ClipboardContent {
    return { ...this.content };
  }
}
