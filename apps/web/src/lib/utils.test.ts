import { describe, expect, it } from 'vitest';
import { cn } from './utils';

describe('cn', () => {
  it('merges class names and resolves tailwind conflicts', () => {
    const result = cn('p-2', 'text-sm', false && 'hidden', 'p-4');
    expect(result).toContain('text-sm');
    expect(result).toContain('p-4');
    expect(result).not.toContain('p-2');
  });
});
