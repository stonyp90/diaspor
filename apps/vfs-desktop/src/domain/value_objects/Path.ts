/**
 * Path Value Object
 *
 * Represents a file system path with validation
 */
export class Path {
  private constructor(private readonly value: string) {
    if (!value || value.trim().length === 0) {
      throw new Error('Path cannot be empty');
    }
  }

  static create(value: string): Path {
    return new Path(value);
  }

  toString(): string {
    return this.value;
  }

  equals(other: Path): boolean {
    return this.value === other.value;
  }

  isDirectory(): boolean {
    return this.value.endsWith('/');
  }

  getParent(): Path | null {
    const parts = this.value.split('/').filter((p) => p.length > 0);
    if (parts.length <= 1) {
      return null;
    }
    parts.pop();
    return Path.create('/' + parts.join('/') + (this.isDirectory() ? '/' : ''));
  }

  getBasename(): string {
    const parts = this.value.split('/').filter((p) => p.length > 0);
    return parts[parts.length - 1] || '';
  }
}
