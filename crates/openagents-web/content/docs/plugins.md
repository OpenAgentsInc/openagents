# Plugins in Coder

Plugins let you extend Coder with custom tools and capabilities. Whether you want to add static analysis for a new language, connect custom linters, or run domain-specific calculations, plugins give Coder access to specialized utilities directly inside your workflow.

---

## What is a plugin?

A plugin is an isolated, single-purpose WebAssembly (Wasm) module paired with a simple JSON manifest. 

Whenever Coder needs specialized functionality that is not built into the core agent, it discovers and calls plugins through a single built-in tool: `capability`.

Plugins in Coder are designed around four principles:

- **Safe and sandboxed:** Plugins run in an isolated WebAssembly environment. By default, a plugin has no network access, cannot write files, and cannot see your filesystem.
- **Explicit permissions:** If a plugin needs to read files from your workspace, it must explicitly declare read-only directory mounts in its manifest. Undeclared access is always blocked.
- **Tamper-proof:** Every plugin manifest includes a cryptographic SHA-256 hash of its compiled WebAssembly binary. If the binary does not match its hash, Coder refuses to load it.
- **Zero local baggage:** Plugins are stateless. Each time Coder calls a plugin, it starts a fresh instance, passes your input, gets the result, and immediately shuts it down.

---

## How plugins work

You do not need to install plugins from an external app store or configure complex background daemons. Plugins live locally on your computer in your plugins catalog directory (`~/.openagents/plugins` or the directory set in `CODER_PLUGINS_DIR`).

```
~/.openagents/plugins/
├── word_stats/
│   ├── manifest.json
│   └── plugin.wasm
└── rust_outline/
    ├── manifest.json
    └── plugin.wasm
```

When you ask Coder to perform a task:

1. **Discovery:** Coder searches its local catalog to see if any installed plugin matches what you are trying to do.
2. **Inspection:** Coder checks the plugin's inputs, outputs, and requested permissions.
3. **Execution:** Coder runs the plugin inside the sandbox with your parameters, enforces time and memory limits, and receives the structured answer.

---

## Permission tiers

To keep your machine safe, Coder categorizes plugins into three permission tiers:

| Tier | What it can access | Prompt required? |
| :--- | :--- | :--- |
| **PureCompute** | No filesystem access and no network access. Plugin only computes results based on the inputs given to it. | No. Runs automatically. |
| **Mounts** | Read-only access to specific directories explicitly declared in its manifest. | Yes. Coder asks for operator approval before running. |
| **Hosts** | Outbound network connections. | Blocked. The capability sandbox does not permit network access. |

---

## Structure of a plugin

Every plugin directory consists of two core files:

1. `manifest.json`: Describes what the plugin does, its inputs and outputs, resource bounds, and required permissions.
2. `plugin.wasm`: The compiled WebAssembly code.

Here is an example `manifest.json` for a word counter plugin:

```json
{
  "schema": "openagents.plugin/v0",
  "name": "word_stats",
  "version": "0.1.0",
  "description": "Compute text statistics: character, word, and line counts.",
  "artifact_path": "plugin.wasm",
  "artifact_digest": "sha256:...",
  "abi_entry": "_start",
  "abi_alloc": "allocate",
  "input_schema": {
    "type": "object",
    "properties": {
      "text": { "type": "string" }
    },
    "required": ["text"]
  },
  "output_schema": {
    "type": "object",
    "properties": {
      "words": { "type": "integer" },
      "lines": { "type": "integer" }
    }
  },
  "mounts": [],
  "hosts": [],
  "timeout_ms": 1000,
  "memory_max_mib": 64
}
```

---

## Getting started with authoring

You can create and manage plugins using the `coder-cli plugin` developer tool suite:

- **Create a project:** Run `coder-cli plugin init my-plugin` to generate a starter template.
- **Build and pin:** Run `coder-cli plugin build my-plugin` to compile the WebAssembly artifact and automatically write the SHA-256 digest into `manifest.json`.
- **Inspect:** Run `coder-cli plugin inspect my-plugin/manifest.json` to verify that imports and schemas match requirements.
- **Test:** Run `coder-cli plugin test my-plugin/manifest.json` to execute test fixtures against the compiled artifact.
- **Install:** Run `coder-cli plugin install my-plugin/manifest.json` to install the plugin into your local catalog.
- **List:** Run `coder-cli plugin list` to verify the catalog and report installed pins.
- **Uninstall:** Run `coder-cli plugin uninstall my_plugin` to remove its pin before deleting unreferenced package bytes. If cleanup fails, retry the same name. See [removing a package](../plugins/plugin-mvp.md#remove-a-package).

---

## Next steps

- Explore existing plugins in the `plugins/` folder of the repository.
- Learn more about the underlying WebAssembly capability model in the developer guides.
