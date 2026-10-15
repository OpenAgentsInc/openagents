# Share a project with a teammate

A project on openagents.com groups chats, files, and notes around your
work, often a GitHub repository. Share one and your teammate sees the same
chats and files, and can add their own. This guide shares a project by
GitHub username, so there's no link to pass along.

Both of you need an openagents.com account. Log in with GitHub at
[openagents.com/login](https://openagents.com/login).

## 1. Have a project

Your projects are on the [Projects page](https://openagents.com/projects) and in the sidebar.
Coder chats you ran in a checkout of a GitHub repository go under your
project for that repository. To make one from the terminal (after
`coder login`):

```sh
openagents projects create "Billing" --repo acme/billing
openagents projects list
```

## 2. Invite your teammate

On the website, open the project from the [Projects page](https://openagents.com/projects). Under
**Members**, type their GitHub username and press **Invite an editor** or
**Invite a viewer**.

Or from the terminal:

```sh
openagents projects invite Billing --github their-username
```

| Role | Can |
| --- | --- |
| Editor (the default) | Read the project, and add chats, files, and notes. |
| Viewer | Read the chats, files, and notes. |
| Owner | Everything, including inviting people. Only the owner invites. |

Add `--role viewer` for a viewer. The invitation works for 7 days unless
you set another time with `--ttl`, such as `--ttl 12h` or `--ttl 30d`
(from 10 minutes to 30 days).

The command and the website also show an invitation link, once. You don't
need to send it when you invited by GitHub username; it's there for
someone whose username you don't know. A project whose files open only on
your own devices can't be shared until you set its files back to
standard.

## 3. Your teammate joins

Your teammate logs in to openagents.com with GitHub and opens their
[Projects page](https://openagents.com/projects). The invitation waits there with **Join** and
**Decline**. Once they join, the project is theirs to open too, marked with their
role, such as **Shared · editor**.

With a link instead, they open it in their browser, or run
`openagents projects join LINK`.

## 4. Manage who's in it

On the project's page, under **Members**, the owner can make someone an
editor or a viewer, make them the owner, or **Remove** them. From the
terminal:

```sh
openagents projects members Billing
openagents projects role Billing their-username viewer
openagents projects remove Billing their-username
```

Giving someone the owner role hands them the project, and you become an
editor. Someone you shared with can leave with **Leave this project**, or
`openagents projects leave Billing`.

## Work in it together

- Start a chat inside the project from the terminal:
  `openagents chat send "Summarize this week's changes" --project Billing`.
  It shows in the project on the website and on the phone, where the
  project's members can read it.
- Move a chat into the project: `openagents projects move CHAT Billing`,
  or from the chat's menu on the website.
- Add a file: `openagents projects files add Billing notes.pdf`.

Next: [Design a part in CAD Lab](/docs/design-a-part).
