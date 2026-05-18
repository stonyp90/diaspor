/**
 * File Size Value Object Unit Tests
 */
import { FileSize } from './FileSize';

describe('FileSize', () => {
  it('should create FileSize from bytes', () => {
    const size = FileSize.fromBytes(1024);
    expect(size.toBytes()).toBe(1024);
  });

  it('should throw error for negative bytes', () => {
    expect(() => FileSize.fromBytes(-1)).toThrow(
      'File size cannot be negative',
    );
  });

  it('should format bytes correctly', () => {
    expect(FileSize.fromBytes(0).format()).toBe('0 B');
    expect(FileSize.fromBytes(512).format()).toBe('512 B');
    expect(FileSize.fromBytes(1024).format()).toBe('1.0 KB');
    expect(FileSize.fromBytes(1536).format()).toBe('1.5 KB');
    expect(FileSize.fromBytes(1048576).format()).toBe('1.0 MB');
    expect(FileSize.fromBytes(1073741824).format()).toBe('1.0 GB');
  });

  it('should compare FileSize instances', () => {
    const size1 = FileSize.fromBytes(1024);
    const size2 = FileSize.fromBytes(1024);
    const size3 = FileSize.fromBytes(2048);

    expect(size1.equals(size2)).toBe(true);
    expect(size1.equals(size3)).toBe(false);
  });

  it('should add FileSize instances', () => {
    const size1 = FileSize.fromBytes(1024);
    const size2 = FileSize.fromBytes(2048);
    const sum = size1.add(size2);

    expect(sum.toBytes()).toBe(3072);
  });
});
