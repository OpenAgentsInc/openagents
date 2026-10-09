# Traces

A trace is a record of an agent run: what you asked, what the agent said,
and each step it took, such as a command it ran, with the result. Coder
keeps every chat as one, in the open ATIF format. Upload a trace to your
openagents.com account to keep it, look through it on the website, or share
it with a link.

## Upload a trace

Sign Coder in to your account once:

```sh
coder login
```

Then upload your latest chat:

```sh
coder trace upload --last
```

Or a chat by its id, or any ATIF file:

```sh
coder trace upload SESSION_ID
coder trace upload --file run.json
```

`openagents coder sessions list` shows your chats and their ids.
`openagents coder trace …` runs the same commands.

Before anything leaves your computer, Coder takes out passwords, API keys,
tokens, your home folder's name, and email addresses, and tells you how many
it took out. The website checks again and refuses a trace that still looks
like it holds a key.

Coder prints the trace's address when it's done. Uploading the same trace
twice keeps one copy. A trace can be up to 8 MB, and your account keeps up
to 100.

## See and share your traces

```sh
coder trace list
```

On the website, open **Settings → Traces**. A trace is private: only you
can see it, signed in. Open it and choose **Share** to get a public link at
`openagents.com/trace/…` that anyone can open; **Stop sharing** turns the
link off. To share as you upload:

```sh
coder trace upload --last --share
```

**Delete** removes a trace from your account, and its link with it.
