## Microcoder

| Aspect | What it does |
| --- | --- |
| Purpose | Hands a concrete programming task to an embedded coding loop, with the requested result and relevant constraints. |
| How you invoke it | Use the `microcoder` tool with a task description. Include the context needed to understand the requested changes. |
| What it executes | 1. Selects a structured next action.<br>2. Runs commands within the current checkout.<br/>3. Returns a result to the conversation. |
| Result you get back | **Model** used for the reply.<br />**Tokens** used by the turn.<BR>**Status** describing how the task ended. |
| Typical use cases | Review `src/microcoder.rs`, update a function, or investigate why a test fails. Long identifiers such as `source_begin_0123456789_abcdefghijklmnopqrstuvwxyz_source_end` remain readable when the terminal is narrow. |

The conversation continues below the table.
