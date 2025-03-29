# Files to Remove from Agents Package

The following files should be removed from the `packages/agents` directory as they are related to the standalone frontend that we don't need in our implementation. We'll be using our existing applications like chatserver and coder to consume the agents service.

## Frontend Files to Remove

### Root Directory
- `index.html` - HTML entry point for the standalone app

### src Directory
- `src/app.tsx` - Main React component for the standalone UI
- `src/client.tsx` - Client-side entry point
- `src/styles.css` - CSS styling for the UI

### Components
- `src/components/` - The entire components directory
  - `src/components/avatar/`
  - `src/components/button/`
  - `src/components/card/`
  - `src/components/dropdown/`
  - `src/components/input/`
  - `src/components/label/`
  - `src/components/loader/`
  - `src/components/menu-bar/`
  - `src/components/modal/`
  - `src/components/orbit-site/`
  - `src/components/select/`
  - `src/components/slot/`
  - `src/components/toggle/`
  - `src/components/tooltip/`

### Other UI-related Directories
- `src/hooks/` - React hooks for the UI
  - `src/hooks/useClickOutside.tsx`
  - `src/hooks/useMenuNavigation.tsx`
  - `src/hooks/useTheme.ts`
- `src/providers/` - React context providers
  - `src/providers/ModalProvider.tsx`
  - `src/providers/TooltipProvider.tsx`
  - `src/providers/index.tsx`
- `public/` - Public assets directory

## Build Configuration to Modify

- `vite.config.ts` - Remove or update to focus only on worker builds
- `tsconfig.json` - Update to remove frontend-specific settings

## Command to Remove Frontend Files

Once you've verified these are the correct files to remove, you can use this command to delete them:

```bash
rm -rf \
  index.html \
  src/app.tsx \
  src/client.tsx \
  src/styles.css \
  src/components \
  src/hooks \
  src/providers \
  public
```

Make sure to run this from the `packages/agents` directory.

## Modified Files

The following files have been modified to support our integration needs:

1. `src/server.ts` - Updated to include CoderAgent routing
2. `src/coder-agent.ts` - Created new specialized agent
3. `src/coder-tools.ts` - Added coding-specific tools
4. `package.json` - Removed UI dependencies
5. `wrangler.jsonc` - Updated for agent service configuration