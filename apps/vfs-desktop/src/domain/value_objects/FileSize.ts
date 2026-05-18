/**
 * File Size Value Object
 *
 * Represents file size with formatting capabilities
 */
export class FileSize {
  private constructor(private readonly bytes: number) {
    if (bytes < 0) {
      throw new Error('File size cannot be negative');
    }
  }

  static fromBytes(bytes: number): FileSize {
    return new FileSize(bytes);
  }

  toBytes(): number {
    return this.bytes;
  }

  format(): string {
    const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
    let size = this.bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }

    return `${size.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
  }

  equals(other: FileSize): boolean {
    return this.bytes === other.bytes;
  }

  add(other: FileSize): FileSize {
    return FileSize.fromBytes(this.bytes + other.bytes);
  }
}
