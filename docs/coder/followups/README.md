# Follow-ups tray (GitHub #13717)

Coder Desktop at 1200 by 800, captured from a built `Coder.app` in a temp `HOME` with a stand-in
worker that speaks the follow-up wire (no model is called).

- `tray_running.png`: a reply runs. "Also cover the empty string" was steered in and shows in the
  message as "Steered into the reply" (that capture predates #13809: a steer now stands in the
  transcript where it was taken, the work before it above and the work after it below, under a
  small "Steer" badge). "And keep the old behaviour for ASCII" waits as a steer, with
  Send now. Two messages are queued, each with Move up, Edit, Remove and Steer. Return always steers
  and Shift-Return queues (File > Queue a Follow-up); Steer and Queue chips send the draft either way.
- `tray_paused.png`: Esc stopped the reply, so the queue paused, with Resume and Clear.
- `tray_restored.png`: Coder opened again on the chat with no worker on it. The kept queue
  (`~/.openagents/coder/sessions/CHAT.queue.json`) shows as it was kept, paused (a queue kept
  waiting, or with a turn running, comes back paused as "Coder closed while these waited").
- `tray_resumed.png`: Resume started a worker on the chat (its turns recalled), and the queue ran
  both messages as turns of the chat.

Drawing: `components/app/followups.coax` (`app_fu_tray`, `app_message_steer`), after Zeron's composer
deck. Page state and presses: `apps/coder-desktop/page/followups.coax`. Host: `Window+FollowUps.swift`.
