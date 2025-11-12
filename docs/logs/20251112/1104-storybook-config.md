• All set. I added Storybook in tauri/, wired it to your Vite/Tailwind setup, mocked Tauri/Ollama calls, and verified the static build works.

  What I changed

  - Storybook config
      - tauri/.storybook/main.ts:1
          - Uses Vite builder with Tailwind plugin.
          - Adds static assets from ../public (makes Berkeley Mono work).
          - Adds aliases:
              - '@' → tauri/src
              - Mocks: @tauri-apps/api/core, @/runtime/adapters/ollama-adapter, @/runtime/useAcpRuntime, @/vendor/assistant-ui/external-store
              - Route local packages to published ones: @openagentsinc/assistant-ui-runtime → @assistant-ui/react, @openagentsinc/react-markdown → @assistant-ui/react-markdown,
                plus CSS mapping.
          - Disables React docgen to avoid chasing local package types.
          - Restricts stories to src/stories/** to avoid pulling in local workspace packages for now.
  - Preview setup
      - tauri/.storybook/preview.tsx:1
          - Imports app CSS and wraps all stories in div.dark so your dark theme applies.
          - Forces the model store to ollama in the Storybook environment to avoid ACP/Tauri usage.
  - Runtime + Tauri mocks (prevent network/native calls)
      - tauri/src/mocks/tauri-api.ts:1
      - tauri/src/mocks/ollama-adapter.ts:1
      - tauri/src/mocks/useAcpRuntime.ts:1
      - tauri/src/mocks/external-store.ts:1
  - Clean up
      - Removed the initial AssistantSidebar story to prevent bundling local workspace packages (we can reintroduce targeted stories later).

  What works now

  - Storybook build succeeds:
      - Ran cd tauri && bun run build-storybook → succeeded; output in tauri/storybook-static.
  - You can start it locally:
      - cd tauri && bun run storybook (dev server at port 6006)

  Notes and rationale

  - Tailwind integration: Storybook’s Vite pipeline includes @tailwindcss/vite, so your @theme and Tailwind 4 layers in App.css compile.
  - Assets: staticDirs: ['../public'] lets /fonts/BerkeleyMono-*.ttf resolve inside Storybook.
  - Local monorepo packages: Your code references local packages in packages/ (e.g., @openagentsinc/assistant-ui-runtime) which pull in more local deps (e.g., assistant-cloud).
    Storybook and the app’s TypeScript build will try to resolve those from repo root, which can fail or bring unrelated errors. For Storybook, I:
      - Aliased @openagentsinc/assistant-ui-runtime to the published @assistant-ui/react.
      - Stubbed code paths (useAcpRuntime, external-store) to avoid touching local workspace internals.
      - Scoped stories to src/stories/** to keep the bundle simple initially.
  - App build: bun run build fails due to existing TypeScript errors in packages/assistant-stream and packages/assistant-ui-runtime. These are unrelated to Storybook and pre-
    existing; I haven’t changed app code to avoid broad refactors.

  How to add your own stories next

  - Create stories near components or under src/stories/ (right now Storybook only loads src/stories/**):
      - For components that require the Assistant runtime, either:
          - Wrap stories with MyRuntimeProvider and keep model set to ollama (works with mocked createOllamaAdapter), or
          - Use lightweight stories that don’t hit the runtime until we finalize local package resolution.
  - If you want stories under src/components/**:
      - Update tauri/.storybook/main.ts:5 stories globs to include those paths.
      - We may need to keep the alias stubs in place to avoid pulling monorepo packages during build.

  Run commands

  - Start Storybook: cd tauri && bun run storybook
  - Build Storybook static: cd tauri && bun run build-storybook

  Open questions / next steps

  - Do you want me to re-enable autodocs and add real stories for AssistantSidebar/Thread once we settle how to resolve the local packages/* in Storybook?
  - If you want bun run build for the app to pass, I can either:
      - Switch imports from @openagentsinc/assistant-ui-runtime back to @assistant-ui/react, or
      - Add the missing dependencies and adjust tsconfig to properly include local packages.
