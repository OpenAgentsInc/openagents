# React 19 + @testing-library/react Compatibility Issue

## Problem Summary
Desktop app tests are failing with React version conflicts when trying to render components using `@testing-library/react`. **All non-rendering tests pass (19/59)**, but **any test that calls `render()` fails (40/59)**.

## Error Message
```
Error: A React Element from an older version of React was rendered. This is not supported. It can happen if:
- Multiple copies of the "react" package is used.
- A library pre-bundled an old copy of "react" or "react/jsx-runtime".
- A compiler tries to "inline" JSX instead of using the runtime.
```

## Current Environment
- **React**: 19.0.0 
- **React-DOM**: 19.0.0
- **@testing-library/react**: 16.3.0 (latest available)
- **Vitest**: 3.2.4
- **@vitejs/plugin-react**: 4.3.4
- **Environment**: JSdom
- **JSX Runtime**: Automatic (React 17+ transform)

## Diagnosis Completed
✅ **No multiple React versions** - Only one react package in node_modules  
✅ **Latest testing library** - @testing-library/react@16.3.0 is current  
✅ **Non-rendering tests work** - 19 logic/mock tests pass perfectly  
✅ **JSX config attempted** - Tried automatic runtime, inline deps  
✅ **Simple test fails** - Even basic `<div>Hello</div>` component fails  

## Technical Context

### Working Test Infrastructure
```javascript
// ✅ WORKS - All 19 tests pass
describe('Logic Tests', () => {
  it('should work with mocks', () => {
    const mockFn = vi.fn();
    expect(mockFn).toBeDefined(); // ✅ Perfect
  });
});
```

### Failing Component Tests  
```javascript
// ❌ FAILS - React Element version error
import { render } from '@testing-library/react';

describe('Component Tests', () => {
  it('should render component', () => {
    render(<SimpleComponent />); // ❌ Fails here
  });
});
```

### Vitest Configuration Attempted
```typescript
export default defineConfig({
  plugins: [react({ jsxRuntime: 'automatic' })],
  esbuild: {
    jsx: 'automatic',
    jsxFactory: undefined,
    jsxFragment: undefined
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./test-setup.ts'],
    server: {
      deps: {
        inline: ['@testing-library/react', 'react', 'react-dom']
      }
    }
  }
});
```

## Research Questions

**Primary Question**: How do I make @testing-library/react@16.3.0 work with React 19.0.0 in a Vitest + JSdom environment?

**Specific Areas to Research**:

1. **React 19 Testing Compatibility**
   - Is @testing-library/react@16.3.0 actually compatible with React 19?
   - Are there known compatibility issues/workarounds?
   - Should we downgrade React or upgrade testing tools?

2. **JSX Runtime Configuration**
   - React 19 changed JSX internals - do tests need different JSX config?
   - Is the "older version" error about JSX transform, not React version?
   - Does Vitest need specific React 19 JSX configuration?

3. **Alternative Solutions**
   - Can we use a different testing approach (e.g., @testing-library/react-native)?
   - Are there React 19 specific testing utilities?
   - Should we mock the rendering entirely for now?

4. **Vitest + React 19 Integration**
   - Are there known Vitest configuration issues with React 19?
   - Do we need different Vite plugins for React 19 testing?
   - Is JSdom the right environment for React 19 tests?

## Impact Assessment
- **PR Status**: Phase 4 testing infrastructure improvements blocked
- **Test Coverage**: 59 total tests, 40 failing due to this issue  
- **Business Logic**: All working (19 logic tests pass)
- **Render Testing**: Completely broken across all components

## Desired Outcome
Get the 40 component rendering tests working so we can achieve **59/59 passing tests** and complete the Phase 4 testing infrastructure improvements.

## Context Files
- Desktop app directory: `/home/christopherdavid/code/openagents/apps/desktop/`
- Vitest config: `apps/desktop/vitest.config.ts`
- Test setup: `apps/desktop/test-setup.ts`
- Package.json: `apps/desktop/package.json`
- Sample failing test: `src/test/simple-render.test.tsx`

---
**Environment**: Bun monorepo, Arch Linux, Tauri desktop app
**Timeline**: Need solution ASAP to unblock PR #1279