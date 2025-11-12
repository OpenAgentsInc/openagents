
• I’ve explored the Tauri app under tauri/ and your Solid/Tauri Storybook example. Here’s a concise plan to add Storybook cleanly without fighting Tauri specifics.

  What I Found

  - Tauri + React + Vite 6 + Tailwind 4 under tauri/ (use bun).
  - Aliases: @ → tauri/src via vite.config.ts (tauri/vite.config.ts:14).
  - Fonts in tauri/public/fonts referenced from CSS root paths (tauri/src/App.css:8).
  - Tauri API is only imported in tauri/src/lib/tauri-acp.ts (tauri/src/lib/tauri-acp.ts:1).
  - Default model is codex (ACP) which will call Tauri; for Storybook we should force ollama or mock the runtime (tauri/src/lib/model-store.ts:14).
  - Ollama adapter makes real HTTP calls (tauri/src/runtime/adapters/ollama-adapter.ts:13). For Storybook, mock it to avoid needing a running server.

  Recommended Setup

  - Location: Put Storybook in tauri/.storybook and run it from tauri/.
  - Builder: Use Vite builder for React (@storybook/react-vite) to match your Vite + Tailwind setup.
  - Static assets: Serve tauri/public so /fonts/... work.
  - Tailwind 4: Add @tailwindcss/vite to Storybook’s Vite config so @theme and Tailwind layers compile.
  - Aliases:
      - @ → ../src so imports like @/components/... resolve.
      - Mock @tauri-apps/api/core to avoid Tauri runtime calls.
      - Mock @/runtime/adapters/ollama-adapter to avoid network calls.
  - Global CSS: Import ../src/App.css in preview to get the app’s variables and theme.
  - Dark mode: Add a decorator that wraps stories with a div.dark so the dark theme applies.
  - Runtime provider: Either:
      - Use your MyRuntimeProvider and switch the model to ollama in a preview decorator (then mock the Ollama adapter), or
      - Provide a fully mocked AssistantRuntimeProvider in a decorator. The first option changes less.

  Key Stubs/Mocks

  - Mock Tauri API so invoke(...) returns predictable values or no-ops.
      - Alias @tauri-apps/api/core to a file like src/__mocks__/tauri-api.ts.
  - Mock Ollama adapter so it streams fake content instead of calling http://127.0.0.1:11434.
      - Alias @/runtime/adapters/ollama-adapter to src/__mocks__/ollama-adapter.ts.
  - Set the model store to ollama inside Storybook so the app uses the local (mocked) runtime:
      - import { useModelStore } from '@/lib/model-store'; useModelStore.getState().setSelected('ollama');

  Example Files To Add

  - .storybook/main.ts
      - Stories globs, staticDirs: ['../public'], framework: '@storybook/react-vite'.
      - viteFinal that:
          - Adds @tailwindcss/vite plugin.
          - Adds alias for '@'.
          - Adds alias for mocks.
      - See similar pattern used in your Solid example: /Users/christopherdavid/code/tauri-start-solid/.storybook/main.ts:1.
  - .storybook/preview.tsx
      - import '../src/App.css'.
      - Decorator to wrap stories with <div className="dark">...</div>.
      - Ensure useModelStore.getState().setSelected('ollama') runs so ACP isn’t used.
  - src/__mocks__/tauri-api.ts
      - Export invoke that returns stub values for create_session, send_prompt, get_session, resolve_acp_agent_path.
  - src/__mocks__/ollama-adapter.ts
      - Export createOllamaAdapter that returns a ChatModelAdapter yielding a short fake stream (e.g., “Hello from mock…”).
  - First story, e.g. src/components/assistant-ui/assistant-sidebar.stories.tsx
      - Use the default export meta and a single story that renders <MyRuntimeProvider><div style={{height: 600}}><AssistantSidebar/></div></MyRuntimeProvider> or rely on global
        decorator if you prefer.

  Install + Scripts (bun)

  - From tauri/:
      - bun add -D @storybook/react-vite storybook @storybook/addon-essentials @storybook/addon-links @storybook/addon-a11y @storybook/test
      - You already have @tailwindcss/vite; reuse it in Storybook’s vite config.
  - Add scripts to tauri/package.json:
      - "storybook": "storybook dev -p 6006 --no-open"
      - "build-storybook": "storybook build -o .output/storybook"

  A Few Gotchas

  - Don’t reuse your app’s vite.config.ts directly in Storybook; its fixed dev server port and Tauri dev host aren’t relevant and may conflict. Instead, selectively add the
    Tailwind plugin and aliases in Storybook’s viteFinal.
  - Importing App.css will apply html, body { overflow: hidden; position: fixed; } inside the preview iframe, which is usually fine. If it interferes, add a tiny override in
    preview.css like html,body{overflow:auto;position:static} and import it after App.css.
  - If TypeScript editor complains about stories, add .storybook to include in tauri/tsconfig.json (optional; Storybook itself doesn’t require it).
  - If you want path alias inference instead of manual alias, you can add vite-tsconfig-paths and include it in Storybook’s viteFinal (mirrors your Solid example). Not strictly
    necessary since you only need '@'.

  Quick Checklist

  - Create .storybook/main.ts, .storybook/preview.tsx under tauri/.
  - Add mock files under tauri/src/__mocks__/.
  - Configure aliases in Storybook to point to mocks and '@'.
  - Import App.css and add a dark wrapper decorator; set model to 'ollama'.
  - Add a first story for a component.
  - Run cd tauri && bun run storybook.

  Want me to scaffold the .storybook config and mock files in tauri/ exactly as outlined and add a starter story, then run Storybook to verify it boots?
