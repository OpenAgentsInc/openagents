# Coder quick start

Coder is an AI coding assistant in your terminal. Ask it to explain code,
change files, or run commands in the folder you open. You can follow its work
in the chat and open a delegated agent's conversation below the input bar.

The new **1.0.0-rc.6** release ships Coder, with the `openagents` command, for
macOS, Linux, and Windows. It includes plugin settings, an OpenRouter model
picker, agent conversations, Markdown replies, code diffs, and saved chats
that you can resume or export.

You do not need an API key to start. Coder uses an available Codex or Claude
Code login, then falls back to the **OpenAgents AI Gateway**. Connecting
OpenRouter is optional.

## Playtest: follow these steps in order

Allow about 15 minutes. Use a new, empty folder so the test stays separate
from your projects.

### 1. Install Coder

On macOS or Linux, run:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, open PowerShell and run:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

Open a new terminal, then check the version:

```sh
coder --version
```

It should report `1.0.0-rc.6`. The [download page](/download) also has manual
downloads. Run the installer again when you want to update.

### 2. Open a test folder

These commands work in a macOS/Linux terminal or Windows PowerShell:

```sh
mkdir coder-playtest
cd coder-playtest
coder
```

You should see an empty chat, the working folder at the top right, and an
input bar at the bottom. Type a message and press Enter to send it. Use
Alt+Enter for a new line. Type `/` to see the available commands.

### 3. Send your first message

Enter:

```text
Say hello, then give me three bullet points about what you can do
in this folder. Do not change any files yet.
```

Check that the reply appears in the chat and the bullet points render
normally. Notice the connection or model information near the input bar.
If a reply needs stopping, press Esc. Ctrl+C quits Coder.

### 4. Make one small change

Enter:

```text
Create hello.txt in this folder containing exactly: Hello from Coder.
Read it back and confirm its contents. Do not change any other files.
```

Wait for it to finish, then enter:

```text
Change hello.txt to say: Hello from Coder RC4.
Show me the before and after. Do not change any other files.
```

Check the file in your editor or file browser. It should contain the second
greeting. Follow the commands and results in the chat.

If a delegated agent appears below the input bar, clear any draft and use
`Up`/`Down` to select its conversation. Check its task, elapsed time, and
token count. Press `Up` from the first agent to return to the main chat. Trackpad or
mouse scrolling should scroll the chat without changing the selected agent.
Some requests run entirely in the main chat and have no separate agent row.

### 5. Try the plugin screen

Enter `/plugins` or press F2. Up/Down selects a plugin, Space turns it on or
off, and Enter opens its settings. Esc returns to the previous screen.

Coder loop, Jev, OpenAgents CLI, and ACP Subagents ship enabled. OpenRouter
BYOK starts off unless a key is already configured. Try turning Jev off and
back on. In `/plugins`, open **ACP Subagents** to see detected local agents and their
checkboxes; an empty list is normal if none are installed.

### 6. Optional: connect OpenRouter and choose a model

Skip this step if you do not have an OpenRouter API key.

1. In `/plugins`, select **OpenRouter BYOK** and press Enter.
2. Enter your key in **OpenRouter API key**. Use `Tab` to select **Save**,
   then press `Enter`. Check that the key is verified.
3. Return to the plugin list and ensure OpenRouter is on.
4. Enter `/models`. Type to search, then use Up/Down and Enter to choose
   `openrouter/free` for the default free router or another listed model.
   Fixed models offer their supported reasoning levels; the free router skips
   that step. Confirm the output limit to save.
5. Send another greeting. Check **OpenRouter connected** near the input bar,
   the selected model, and the actual model above the reply. A fixed model's
   label includes its reasoning setting, such as `:low`.

Plugin settings persist when you quit and reopen Coder. Turning OpenRouter
off retains its saved key and returns to the other available providers.

### 7. Export and resume

Enter `/export` after the reply finishes. Coder saves an **ATIF** file: a
record of the conversation and its calls. It shows the file path and copies
the path to your clipboard when clipboard access is available.

Press Ctrl+C, run `coder` again in the same folder, and enter `/resume`.
Select your test chat with Up/Down, then press Enter. The messages should
return without repeating earlier commands. Send:

```text
What did we change in hello.txt?
```

### 8. Share your results

Report your operating system, Coder version, provider and model, the step
you tried, and what happened. Include a screenshot or the exported test chat
if helpful. Keep API keys and private project content out of your report.
You can [report a problem on GitHub](https://github.com/OpenAgentsInc/openagents/issues).

## Use Coder from a script

The companion CLI exposes the same chat runtime:

```sh
openagents coder chat -p "Explain the files in this folder. Do not change anything." --session first-test --json
```

Reuse `--session first-test` to continue that chat. Run
`openagents coder --help` for the other commands.
