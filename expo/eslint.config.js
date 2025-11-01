// https://docs.expo.dev/guides/using-eslint/
const { defineConfig } = require('eslint/config');
const expoConfig = require('eslint-config-expo/flat');

module.exports = defineConfig([
  expoConfig,
  {
    ignores: ['dist/*'],
  },
  {
    files: ['components/**/*.{ts,tsx}'],
    plugins: {
      unicorn: require('eslint-plugin-unicorn').default
    },
    rules: {
      // Enforce PascalCase filenames for components per ADR-0006 (Ignite style)
      'unicorn/filename-case': ['error', {
        cases: { pascalCase: true },
        // Allow index files
        ignore: [/^index\.[jt]sx?$/]
      }],
    },
  },
]);
