import { defineConfig, globalIgnores } from 'eslint/config';
import convexPlugin from '@convex-dev/eslint-plugin';
import js from '@eslint/js';
import tseslint from 'typescript-eslint';

export default defineConfig([
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...convexPlugin.configs.recommended,
  {
    rules: {
      // The codebase is mid-migration; allow `any` in transitional adapters and protocol surfaces.
      '@typescript-eslint/no-explicit-any': 'off',
      '@typescript-eslint/no-empty-object-type': 'off',
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
    },
  },
  globalIgnores(['convex/_generated', 'dist']),
]);
