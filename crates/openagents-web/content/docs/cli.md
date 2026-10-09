# The openagents command

`openagents` is the command-line program behind OpenAgents Terminal. Use it
to chat from a script, change Coder's settings, work with plugins, and
connect phones to a computer without a screen. It installs with
OpenAgents Terminal; see [Download](/docs/download).

Every command takes `--json` for machine-readable output and `--help` for
its syntax. `openagents doctor` shows which identities, stores, and relays
it uses.

## Chat

```sh
openagents chat "How do I connect a phone?"     # a new thread
openagents chat --thread ID "And on Android?"   # continue a thread
echo "What is the Gym?" | openagents chat -     # the message from stdin
openagents chat threads                         # your threads, newest first
openagents chat read --thread ID                # a whole thread
openagents chat export --thread ID              # a thread as a trajectory file
```

The reply streams to standard output, and everything else (the thread ID,
offers, suggestions) goes to standard error, so `reply=$(openagents chat
"…")` captures only the reply.

When a message is coding work, Coder runs on this computer at once, in the
Git checkout you run the command in, and its steps stream to the terminal.

```sh
openagents chat send --no-run "add a test for slugify"   # only offer Coder
openagents chat run-coder --thread ID                     # accept the offer later
openagents chat follow --thread ID                        # watch a run
openagents chat answer --thread ID "yes, use the smaller fix"
openagents chat stop --thread ID
openagents chat work --issues 101,102                     # work GitHub issues
```

Ctrl-C stops following a run; the run keeps going. See
[Coder](/docs/coder) and [Work on a GitHub issue](/docs/github-issues).

When the Mac app (or another host) runs on this computer, these are the
same threads it shows. Without one, the command keeps its threads in
`~/.openagents/chat/`, and they move into the host's store the first time a
host runs. `--scratch` uses a throwaway identity and thread.

## Settings

```sh
openagents settings show
openagents settings set coder.start ask_first
openagents settings unset coder.start
```

Every key is in the [settings reference](/docs/settings).

## Plugins

```sh
openagents plugin list                          # published plugins
openagents plugin run DIR --in PROJECT --request "..."
openagents plugin test init                     # write tests with us
openagents plugin test run .                    # test with and without
openagents plugin test publish REPORT           # add a result to the Gym
openagents plugin test check RESULT_ID          # rerun someone's result
openagents plugin defaults sync                 # the plugins Coder uses for everyone
```

See [Plugins](/docs/plugins) and the guides after it.

## Models

Call any model on the [OpenAgents API](/docs/api) with your key in
`OPENAGENTS_API_KEY`:

```sh
openagents inference google/gemini-3.8-flash "Say hello."
openagents inference openagents/chat --input "Tell me a story." --stream
openagents inference openagents/fast --json '{"input": "Hi", "temperature": 0.2}'
openagents inference models                     # every model and its price
openagents inference rates                      # the rate card
```

`--format json` prints the whole answer and `--format events` each streamed
event. With no key, `--pay x402 --max-msat N` pays for one request from
your wallet ([Pay per request](/docs/api/pay-per-request)).

## Connect

For a computer without the Mac app, or to script pairing. The host must be
running.

```sh
openagents connect invite            # a QR code for the phone, in the terminal
openagents connect devices           # the phones that can reach this computer
openagents connect remove DEVICE     # cut a phone off now
openagents connect status            # this computer's host at a glance
openagents connect --ssh me@box      # set up a computer you reach over SSH
```

`connect --ssh` installs `openagents` on the other computer, starts its
host, and pairs it with this one. Set up SSH key login first.

On Windows, `connect` and a few other commands say they need macOS or
Linux.

## More

The command also covers the Verse, XP, keys, relays, a Lightning wallet for
agents, and more. `openagents --help` lists every group, and the full
reference is on
[GitHub](https://github.com/OpenAgentsInc/openagents/blob/main/docs/cli/README.md).
`openagents mcp serve` offers the command to other agents over MCP.

Next: [Chat](/docs/chat).
