/**
 * Path Value Object Unit Tests
 */
import { Path } from './Path';

describe('Path', () => {
  it('should create Path from string', () => {
    const path = Path.create('/home/user');
    expect(path.toString()).toBe('/home/user');
  });

  it('should throw error for empty path', () => {
    expect(() => Path.create('')).toThrow('Path cannot be empty');
    expect(() => Path.create('   ')).toThrow('Path cannot be empty');
  });

  it('should detect directories', () => {
    expect(Path.create('/home/user/').isDirectory()).toBe(true);
    expect(Path.create('/home/user').isDirectory()).toBe(false);
  });

  it('should get parent path', () => {
    const path = Path.create('/home/user/file.txt');
    const parent = path.getParent();
    expect(parent?.toString()).toBe('/home/user');
  });

  it('should return null for root path parent', () => {
    const path = Path.create('/');
    expect(path.getParent()).toBeNull();
  });

  it('should get basename', () => {
    expect(Path.create('/home/user/file.txt').getBasename()).toBe('file.txt');
    expect(Path.create('/home/user/').getBasename()).toBe('user');
  });

  it('should compare paths', () => {
    const path1 = Path.create('/home/user');
    const path2 = Path.create('/home/user');
    const path3 = Path.create('/home/other');

    expect(path1.equals(path2)).toBe(true);
    expect(path1.equals(path3)).toBe(false);
  });
});
