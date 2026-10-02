# Privacy prompts: nothing OpenAgents runs asks for them

macOS asks the person before a program reads their music, photos, mail,
messages, documents, another app's data, or a removable volume, or sends
Apple Events to another app (Transparency, Consent, and Control, "TCC").
The dialog names the *responsible* program. For everything the Coder host
starts (Coder runs and their commands, whole coding agents such as Grok
Build, terminals, background rules, plugin evaluations, session import) that is the
host, so a stray `du ~/*` reads as "Coder would like to access Apple
Music". Nothing OpenAgents does needs those places, so nothing goes there.

## What happened (2026-10-02)

At 13:28 the owner's Mac showed "Coder would like to access Apple Music".
The unified log has `sandboxd` asking `tccd` for approval four times
between 13:28:18 and 13:28:48 and then `kTCCServiceMediaLibrary granted by
TCC for coder`. No Coder run's commands touched the home folder in that
window. The host's disk cleanup rule did: the disk was nearly full, so
planning the 13:30:08 run measured the rule's report-only folders,
`~/Library/Caches` and `/private/var/folders`, and that walk went into
`~/Library/Caches/com.apple.Music`, `com.apple.AMPLibraryAgent`, and the
other caches of Apple's own apps, which macOS guards as the media library
and the apps' private data
([#10214](https://github.com/OpenAgentsInc/openagents/issues/10214)).

## How it is kept from happening

The list lives in one place, `crates/coder-boundary/src/privacy.rs`
(`coder_boundary::privacy`), and is macOS-only; on Linux and Windows
nothing is protected this way and every helper returns its input.

- **Protected places.** Under the home: Desktop, Documents, Downloads,
  Music, Movies, Pictures; Mail, Messages, Safari, Calendars, Reminders,
  HomeKit, Photos, Cookies, Accounts, Biome, Suggestions, call history,
  Contacts (`AddressBook`), Knowledge, FaceTime, device backups
  (`MobileSync`), the TCC database; every app's container (`Containers`,
  `Group Containers`); iCloud Drive (`Mobile Documents`) and cloud storage
  (`CloudStorage`); and the caches of Apple's media and personal apps
  (`Library/Caches/com.apple.Music`, `com.apple.iTunes`, `CloudKit`, ...).
  Outside the home: `/Volumes`.
- **The host's own walks.** `background`'s measuring
  (`paths::measure`) never enters a protected place and refuses to measure
  one; its report-only measuring (`paths::measure_report`, the "Largest
  not cleaned" sizes) also leaves out every `com.apple.*` and `CloudKit`
  folder; a rule's glob never lists or matches one; and `Layout::refuse`
  refuses to clean one, or a folder that holds one. Session import reads
  only `~/.claude` and `~/.codex`, and not at all when either is a link
  into a protected place.
- **Every command under a boundary.** A `coder_boundary::Boundary` profile
  ends with the privacy rules: reads and writes in every protected place
  denied (the folder's own name stays visible, so `ls ~` works), Apple
  Events, the camera, and the microphone denied. A checkout, readable, or
  writable path the caller put inside a protected place on purpose is
  allowed back. The rules come last, so they win over the profile's
  earlier allows.
- **Full access and whole coding agents.** A full-access command (the
  owner's own host, `coder host autostart on --full-access`) runs under
  `privacy::command`: `sandbox-exec` with a profile that allows everything
  but the privacy rules, with the workspace allowed back. Grok Build,
  OpenCode, and Devin processes run under the same profile
  (`microcoder::repository::private_spec`), and so does a terminal's
  shell or command (`coder-pty`), which a person drives from the phone
  with nobody at the Mac to answer a dialog. Approve-everything stays
  approve-everything everywhere else: the worktree, the repository,
  `~/.openagents`, toolchains (`~/.cargo`, `~/.rustup`, Nix, Homebrew),
  `/tmp`, and the engines' own `~/.codex`, `~/.claude`, `~/.grok`, and
  `~/.config` are untouched.
- **Boundaries inside it.** `sandbox-exec` cannot apply a profile inside
  another, so the privacy-only profile lets `/usr/bin/sandbox-exec` start
  outside itself (`(with no-sandbox)`). A Coder boundary, a plugin
  evaluation, or Codex's own sandbox started by a full-access command
  therefore still works, and every Coder boundary carries the privacy
  rules itself. A sandboxed process may not start a set-user-ID program,
  so the system's (`ps`, `top`, `sudo`, `crontab`, ...;
  `privacy::SETUID_PROGRAMS`) start outside it the same way and work as
  they do for the owner. A `Boundary` profile never has these escapes.

A sandbox denial is decided by the command's own profile before the
system's privacy check, so the command gets an ordinary "Operation not
permitted" and no dialog appears. The model is told so in its
environment note under full access, and adapts instead of retrying.

## Limits

- The list covers what macOS guards by path. Prompts that are not about a
  path (Contacts or Photos through their frameworks, location, screen
  recording, accessibility, local network) are not denied by path; none
  of them is reached by a shell command or a file walk, and AppleEvents,
  camera, and microphone, the ones a command can reach, are denied.
- A full-access command that runs `sandbox-exec` itself with a profile of
  its own runs under that profile only. That is how Codex's and Coder's
  boundaries keep working; it is not a way around the owner's own choice.
- A decision the owner already made in System Settings stays. Nothing
  here reads or changes the TCC database.

## Checked by

- `coder_boundary::privacy` unit tests: the list, the report-only skip,
  rule order and allow-back, wrapping once.
- `crates/coder-boundary/tests/privacy.rs` (macOS): with a made-up home,
  a privacy-sandboxed `cat`, `ls`, and `touch` in its `Music` get
  "Operation not permitted" while the rest of the home works; a checkout
  in `Documents` is allowed back; a boundary still starts inside the
  privacy sandbox; every `Boundary` profile denies the real protected
  places and has no escape. The real `~/Music` is never touched.
- `background::paths` test
  `privacy_protected_folders_are_never_walked_or_cleaned`.
- `coder-host` test
  `sessions_behind_a_link_into_a_protected_folder_are_not_read`.
