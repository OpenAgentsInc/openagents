//! The Account tab's own screens: this device's identity keys and the
//! changelog.
//!
//! The device key is a secp256k1 key the platform host creates from its
//! secure random source and keeps in its protected store (Keychain on iOS).
//! It is not derived from a NIP-06 seed phrase, so no mnemonic can restore
//! it. Keys are shown in NIP-19 form first (`npub`, `nsec`), with hex beside
//! the public key. The secret key leaves this crate only in the answer to an
//! explicit reveal; the app packet never carries it.

use secp256k1::{Secp256k1, SecretKey};
use serde::Serialize;

/// One TestFlight build and what it brought.
#[derive(Serialize)]
pub struct Release {
    pub version: &'static str,
    /// The build number, or the range of builds the entry covers.
    pub build: &'static str,
    pub title: &'static str,
    /// Where testers should look in this build: every build has one.
    pub what_to_test: &'static str,
    pub items: &'static [Item],
}

/// One change: a short name and a sentence about it.
#[derive(Serialize)]
pub struct Item {
    pub title: &'static str,
    pub detail: &'static str,
}

/// The app's builds, newest first. The first entry's build is the one
/// `bins/openagents-ios/host/project.yml` builds, so every TestFlight build
/// ships with its own entry and its What to test line.
pub const CHANGELOG: &[Release] = &[
    Release {
        version: "1.0.0",
        build: "54",
        title: "OpenAgents 1.0 beta",
        what_to_test: "No sign-in is needed. In Chat, ask anything or tap a suggested question: the reply should stream in. Open Wallet: it should show your balance, Receive, and Send. In Account, open each row: Computers, Appearance, Your keys, Identity keys, About this device, Changelog, and Report a problem. To reach your own computer, get OpenAgents at https://openagents.com/download, run openagents connect invite there, and scan the code from Account > Computers.",
        items: &[
            Item {
                title: "Three tabs",
                detail: "The app is Chat, Wallet, and Account; the features still in development are hidden.",
            },
            Item {
                title: "Plainer words",
                detail: "Your keys and Connect a computer say what to do in plain words, with the download link.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "53",
        title: "The world opens again",
        what_to_test: "Open the Verse tab and check that the world loads instead of an \"Invalid native Verse configuration\" error. Walk through the EVERGLADE arch and check there is no studio banner at the top when no computer is online. Everything from build 52 applies: the town clock, Mira, Tobin, and Wren, swimming and rain, and the Civic Hall and Agora.",
        items: &[
            Item {
                title: "Verse starts",
                detail: "Build 52's world refused a setting the app sent and showed an error; the world now opens.",
            },
            Item {
                title: "Quieter studio",
                detail: "No banner appears in Everglade when no computer is online for the studio.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "52",
        title: "Everglade's day, water, and townsfolk",
        what_to_test: "Walk through the EVERGLADE arch. Watch the sky change as the town clock runs: a town day lasts one real hour, with dusk and lamp-lit night. Find Mira, Tobin, and Wren going about their day around the Market Hall. Walk into Lantern Pond and swim, then dive and look up at the surface from below. If it rains, check the ripples on the ponds and the wet ground. Visit the Civic Hall and the Agora. Check that all text uses the new monospace font. Leave through THE GRID arch.",
        items: &[
            Item {
                title: "Town clock",
                detail: "Everglade runs a day and night cycle, mostly daylight, with lamps at night and readable darkness.",
            },
            Item {
                title: "Townsfolk",
                detail: "Mira the baker, Tobin the smith, and Wren the bell-ringer keep daily routines and pass a rumor at midday.",
            },
            Item {
                title: "Water",
                detail: "Swimmable ponds and stream with breath, waves, reflections, an underwater view with caustics, and rain that ripples the water and wets the ground.",
            },
            Item {
                title: "Buildings",
                detail: "The Greco-futurist Civic Hall, belvedere, and Agora join the town.",
            },
            Item {
                title: "Paper Mono",
                detail: "Every screen now uses the Paper Mono font.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "51",
        title: "Everglade's light, sky, and spells",
        what_to_test: "Walk through the EVERGLADE arch. Check the blue sky with clouds and a sun, distant hills fading into haze, shadows reaching across the glade, and the workshop interior shaded rather than flat. Use the icon hotbar above the sticks: Levitate, then hold Up or Down to climb, and try Feather Fall, Wall of Stone, Wind Wall, and Reverse Gravity. Walk sideways and backward to see the strafe and backpedal. Leave through THE GRID arch.",
        items: &[
            Item {
                title: "Sky and lighting",
                detail: "A daylight sky lights the glade, with height fog, baked shading indoors and under trees, and sun shadows that follow you.",
            },
            Item {
                title: "Icon hotbar and spells",
                detail: "The chamber's icon bar with Levitate, Up, Down, and four spells; hold Up or Down to keep climbing.",
            },
            Item {
                title: "Walking",
                detail: "The original library's walk and jog, strafing, backpedaling, and landing on roofs.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "50",
        title: "Movement controls in Everglade",
        what_to_test: "Walk through the EVERGLADE arch. Above the sticks, tap Jump, switch Sprint back to Run, and tap Levitate. Use Up and Down to change height, then Land to descend gently. Keep a movement stick held while tapping the bar. Leave through THE GRID arch and check the bar disappears.",
        items: &[Item {
            title: "Everglade movement hotbar",
            detail: "Jump, sprint, levitate, change altitude, and land from touch controls above the sticks.",
        }],
    },
    Release {
        version: "1.0.0",
        build: "49",
        title: "Everglade grows a street",
        what_to_test: "Open Account, then Changelog, and check build 49 is first. Walk into Everglade through the EVERGLADE arch. Around the yard you now find three new buildings: a cottage with a chimney to the west, an open café pavilion with tables to the east, and a reading room behind the strongroom. Walk around each one and check you can't walk through their walls, and that every station still reaches as before.",
        items: &[Item {
            title: "Three new buildings in Everglade",
            detail: "A cottage, a café pavilion, and a reading room around the yard, a first step toward the Everglade city map.",
        }],
    },
    Release {
        version: "1.0.0",
        build: "48",
        title: "The whole screen for Everglade",
        what_to_test: "Open Account, then Changelog, and check build 48 is first. Walk through the EVERGLADE arch on the Grid: while it downloads the panel still shows progress and Cancel, but inside Everglade there is no black panel at all, only the glade and your sticks. Leave by walking back through the arch lettered THE GRID behind where you arrive.",
        items: &[Item {
            title: "No panel over Everglade",
            detail: "Inside Everglade the screen is all world. Walk back through THE GRID arch to leave.",
        }],
    },
    Release {
        version: "1.0.0",
        build: "47",
        title: "Everglade on your phone",
        what_to_test: "Open Account, then Changelog, and check build 47 is first. Walk through the EVERGLADE arch on the Grid and walk up to the workshop's stations: the caption names the station and never asks you to press a key.",
        items: &[Item {
            title: "No keyboard hints on your phone",
            detail: "Everglade's captions no longer ask you to press F.",
        }],
    },
    Release {
        version: "1.0.0",
        build: "46",
        title: "Everglade",
        what_to_test: "Open Account, then Changelog, and check build 46 is first. Open Verse and walk through the arch lettered EVERGLADE: the first visit downloads the glade, with progress, Cancel, and Retry. In Everglade you play the hooded ranger, with no spade beside you; walk the path to the workshop, the yard, and the hall. Come back by walking through the arch lettered THE GRID or tapping The Grid, and check you return beside the Everglade arch with other players visible again. A second visit should open without downloading.",
        items: &[
            Item {
                title: "Everglade",
                detail: "Walk through the EVERGLADE arch on the Grid to visit a forest glade and its workshop, where you will work with a team of coding agents.",
            },
            Item {
                title: "A ranger in the glade",
                detail: "In Everglade you play an outfitted ranger instead of the Grid's figure.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "45",
        title: "Your wallet on your computers",
        what_to_test: "Open Account, then Changelog, and check build 45 is first. On a computer connected to this phone, run openagents wallet link: the phone should ask \"Use your wallet on ...?\" with the same six-digit code the computer shows. Check the codes match, tap Approve, and confirm with Face ID; the computer should then say your wallet is on it, with the same balance as the phone. Open Account, then Computers: an online computer that runs background watchers lists them under its status. Open Account, then Your keys, add a key, and tap Test. Long-press text in a chat and try Give feedback.",
        items: &[
            Item {
                title: "Your wallet on your computers",
                detail: "Run openagents wallet link on a computer, check the six-digit code matches, and tap Approve. The computer then uses the same wallet and balance as this phone.",
            },
            Item {
                title: "Your own keys",
                detail: "Account, then Your keys: add your own OpenRouter, Vercel AI Gateway, or TypeSafe key, test it, and turn on Use my keys for everything to run chat on your keys.",
            },
            Item {
                title: "Background watchers on Computers",
                detail: "Each online computer in Computers lists the background watchers it runs.",
            },
            Item {
                title: "Feedback on any text",
                detail: "Select text in a chat and tap Give feedback to tell us what's wrong or what should change.",
            },
            Item {
                title: "Coder that stopped unexpectedly",
                detail: "When a Coder run's process ends unexpectedly, the phone says so, and the next run in that project starts normally.",
            },
            Item {
                title: "Suggestions in every new chat",
                detail: "A new chat always shows four suggestions, even after you've used them all.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "44",
        title: "Shipped from the phone",
        what_to_test: "Open Account, then Changelog, and check build 44 is first. Start a Coder run from the phone: its card should show how long Coder has worked and, when it finishes, how the run ended. With a computer connected, ask which coding agents are connected: the reply should name your computer's agents. Use chat and run a plugin test without usage limits.",
        items: &[
            Item {
                title: "Shipped from the phone",
                detail: "Coder made and uploaded this build from the phone.",
            },
            Item {
                title: "Coder's time and outcome",
                detail: "The Coder card now shows how long Coder has worked and how the run ended.",
            },
            Item {
                title: "Your computer's coding agents",
                detail: "Asking what coding agents are connected names your computer's agents.",
            },
            Item {
                title: "No usage limits",
                detail: "There are no usage limits.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "43",
        title: "Coder starts at once",
        what_to_test: "With a computer paired and its desktop app set to start Coder at once (the default), ask the chat for a small coding change: Coder should start on the computer without a Run Coder tap, and the chat should show where it runs with a Stop button. Open that Coder chat while it works: each step should read as a short line, like \"Read README.md\", never as raw code. Ask \"What is a capability claim?\": the answer should come from our essays.",
        items: &[
            Item {
                title: "Coder starts at once",
                detail: "When a reply offers Coder and your computer starts Coder at once, it starts there with no Run Coder tap. The chat shows where it runs, with Stop. A computer set to ask first still shows Run Coder.",
            },
            Item {
                title: "No approval questions",
                detail: "Coder on your computer approves its own steps by default, including a push, so a run no longer stops to ask you.",
            },
            Item {
                title: "Readable steps",
                detail: "Steps from Grok Build, Devin, and OpenCode show as short lines, like \"Read README.md\" or \"Ran cargo test\", instead of raw tool arguments.",
            },
            Item {
                title: "Answers from our essays",
                detail: "Questions about Test-Time Capabilities and The Return of the General Agent are answered from the essays, with their links.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "42",
        title: "Text only",
        what_to_test: "On the Chat tab, in a new chat and in an open one, the message box should have no attach button above it, and tapping the box twice should not offer Paste for a copied photo. Open the previous chats: the chat you started last should be at the top, with its project shown as a label on the row. With a computer paired, after a Coder run finishes, open its chat and ask \"What did you change?\": the reply should answer in the chat. Then say \"Now add a test\": Coder should continue the same task.",
        items: &[
            Item {
                title: "Text only",
                detail: "The phone chat takes text only for now. The attach button above the message box is gone, and a photo can't be added to a message by picking or pasting it. The desktop app is text only too.",
            },
            Item {
                title: "Follow-ups after a Coder run",
                detail: "Once a Coder run has finished, a question about it, like what changed, is answered in the chat. Asking for more work continues the same Coder task on your computer.",
            },
            Item {
                title: "Newest chat on top",
                detail: "In the previous chats, the chat you started last is always at the top, whatever its project, with the project shown as a label on its row. Pinned chats stay above, and archived ones below.",
            },
            Item {
                title: "Grok Build on your computer",
                detail: "Coder on your computer can use Grok Build by default, after Codex and Claude Code, when it is installed and signed in.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "41",
        title: "Plugins",
        what_to_test: "On the Chat tab's menu, the first chip should say Test a plugin; tap it and the reply should offer plugins to test, with cards that say plugin. In a new chat, ask \"Which plugins can I test?\": the reply should name all six, Project map, Code finder, Test reader, Explain this error, Release notes, and Dependency check. Ask \"Can you book me a flight to Tokyo next week?\": the reply should say there's no plugin for that yet, under NO PLUGIN FOR THAT YET with ADD A PLUGIN. With a computer paired, ask \"List my plugins\": the reply should show a card with Run, and tapping it should list the plugins on your computer.",
        items: &[
            Item {
                title: "Say plugin",
                detail: "Anything you add to OpenAgents is now called a plugin: Test a plugin on the Chat tab's menu, with and without the plugin in a test, and NO PLUGIN FOR THAT YET with ADD A PLUGIN when you ask for something nothing does yet.",
            },
            Item {
                title: "Three new plugins in the Gym",
                detail: "Explain this error finds the line behind a failing command's output and says the likely cause and fix. Release notes groups a git log into breaking changes, features, and fixes. Dependency check flags duplicate versions, loose version ranges, and licenses your project doesn't allow. You can test each one from chat like the others.",
            },
            Item {
                title: "The chat knows every plugin",
                detail: "Ask which plugins are in the Gym, or which you can test, and the reply names all six. Ask what one of them does and it answers from that plugin's own description.",
            },
            Item {
                title: "List my plugins",
                detail: "With a computer paired, asking to list your plugins offers a card that lists the plugins on that computer when you tap Run.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "40",
        title: "Coder runs the engine you asked for",
        what_to_test: "With no computer paired, send \"Do a test delegation to claude\" in a new chat: the reply should say it needs a computer and offer Connect a computer, with no Gym test. With the latest OpenAgents on a paired computer, send \"Do a test delegation to claude\" in a new chat on the phone and tap Run Coder: Coder should start on Claude Code, or its start card should say why another engine is running, and the run should do a small check of your project instead of trying to sign in to Claude. Then open one of the computer's chats from the previous chats and ask \"What's the working directory right now?\": the reply should answer it, with no Coder run starting.",
        items: &[
            Item {
                title: "Your engine, from any chat",
                detail: "When you ask for Claude, Codex, or another coding engine and tap Run Coder in a chat on the phone, your computer now starts that engine, not only in the computer's own chats. If that engine isn't available, the start card names the one running instead. This needs the latest OpenAgents on your computer.",
            },
            Item {
                title: "Delegations do the work",
                detail: "When you hand a task to Claude or another engine, the run does your task. It no longer treats the request itself as the work and gets stuck trying to sign in to that engine; a request with nothing more to do gets a small, harmless check of your project.",
            },
            Item {
                title: "Answers stay answers",
                detail: "When a reply already answers your question, Coder no longer starts on your computer as well, and the reply no longer offers Run Coder.",
            },
            Item {
                title: "Big projects start",
                detail: "Coder now starts in projects with many folders. Before, your computer could refuse to start Coder there. This needs the latest OpenAgents on your computer.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "39",
        title: "Ask for Claude by name",
        what_to_test: "With no computer paired, send \"Do a test delegation to claude\" in a new chat: the reply should say it needs a computer and offer Connect a computer, with no Gym test. With a computer paired, ask \"What can you do?\": the reply should not ask you to connect a computer. With the latest OpenAgents on that computer, open one of its chats from the previous chats, send \"Do a test delegation to claude\", and tap Run Coder: Coder should start on Claude Code, or its start card should say why another engine is running. The run should show your message once, with \"Continued from the OpenAgents app\" as a note under it.",
        items: &[
            Item {
                title: "Name the engine you want",
                detail: "When you ask to hand work to Claude, Codex, or another coding engine and a computer can take it, the reply says which one you asked for. In a computer's chat, Run Coder starts on that engine, and if it can't, because it isn't signed in or has reached a limit, the start card says so and names the one running instead. This needs the latest OpenAgents on your computer.",
            },
            Item {
                title: "Your message, once",
                detail: "When Coder starts from a chat, its run shows your message once, with where it came from as a small note, instead of repeating it as \"Continued from the OpenAgents app\".",
            },
            Item {
                title: "The chat knows your computer",
                detail: "With a computer paired, the chat knows which one, even while it is offline, so it no longer asks you to connect a computer you already have.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "38",
        title: "Coder starts on what you asked",
        what_to_test: "In a new chat, ask \"Who can you delegate to?\", then send \"Do a test delegation now\". The second reply should offer Coder (Run Coder with a computer paired, or Connect a computer without one) and should not also show a Gym test. With a computer paired, tap Run Coder: the run should be titled by \"Do a test delegation now\", not by the chat's first message, and its transcript should show no internal judge rows. On the Coder tab, chats from a task's worktree should be listed under the project's own name.",
        items: &[
            Item {
                title: "One reply, one next step",
                detail: "A reply that offers a Gym test no longer also starts Coder. When you ask to hand work off, the reply offers Coder, or Connect a computer if none is ready.",
            },
            Item {
                title: "Coder starts on your request",
                detail: "Run Coder now titles and starts the run with the message that asked for the work, plus a few earlier messages for context, not the chat's first message or the reply after it.",
            },
            Item {
                title: "No internal decision rows",
                detail: "Checks Coder makes for itself while it works stay in its record and no longer show as rows in the transcript.",
            },
            Item {
                title: "Usage limits that match",
                detail: "When your computer's newer usage reading shows a limit has reset, Coder's start card no longer says that engine is limited or names it as a fallback. This needs the latest OpenAgents on your computer.",
            },
            Item {
                title: "Projects keep their names",
                detail: "Chats run in a task's working copy are listed under the repository's name instead of the working copy's folder. This needs the latest OpenAgents on your computer.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "37",
        title: "Words and screenshots together, and review before you publish",
        what_to_test: "Tap the paperclip, pick a photo, type a question that isn't about code, such as \"What is OpenAgents?\", and tap Send: your words should send and get a reply, and the photo should stay above the message box with a line saying images go only to Coder. With a computer paired, attach a screenshot, ask for a code change, and tap Run Coder on the reply: the run should start with the screenshot. When a Coder run on your computer finishes, open it: its card should count the files changed and name the base and head it compares, What changed should open the diff, and Publish should make one draft pull request or commit and link it on the card. If the change moves on the computer after you open it, the card should say so and offer Refresh.",
        items: &[
            Item {
                title: "Send words and photos together",
                detail: "A message with photos now sends: its words go to chat and get a reply, and the photos stay in your draft for Coder.",
            },
            Item {
                title: "Screenshots reach Coder",
                detail: "Tap Run Coder after a coding reply and the photos in your draft go to Coder on your computer as they are, so Codex and Claude Code runs can see them.",
            },
            Item {
                title: "Photos stay when a reply doesn't start Coder",
                detail: "Images go only to Coder. If the reply doesn't lead to Coder, your photos stay in the draft with a line that says so.",
            },
            Item {
                title: "Review a finished change",
                detail: "When a Coder run on your computer finishes, its card names the exact revisions it compares, and What changed opens the diff between them. If the change moves after you open it, the card says so and offers Refresh.",
            },
            Item {
                title: "Publish once",
                detail: "On a computer you own or may operate, Publish on that card makes a draft pull request or a commit, as the project is set up, just once, and the card links to it.",
            },
            Item {
                title: "Some messages still take words only",
                detail: "A message to a running Coder task or to your computer's own thread carries words only. If your draft has photos, it stays in the message box with a line that says why.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "36",
        title: "Photos, chat menus, and a better message box",
        what_to_test: "In the message box, type an emoji and an accented letter such as é, then tap delete: each should go in one tap. Double-tap the text and choose Undo, then Redo. Tap the paperclip and pick a photo: it should show above the message box, and tapping Send should say chat takes text only while your text and photo stay in the box; remove the photo and the message should send. Touch and hold a saved chat in Chats: a menu should offer Pin and Archive, and the chat should move when you pick one; hold it again to unpin it. Ask for a short code sample: the code should be colored. Send a few messages at a normal pace: none should say you're sending too quickly.",
        items: &[
            Item {
                title: "The message box edits like the desktop app",
                detail: "Delete removes a whole emoji or accented letter at once, and Undo and Redo are in the message box's edit menu.",
            },
            Item {
                title: "Menus on saved chats",
                detail: "Touch and hold a chat in Chats to pin or unpin it, or archive or restore it.",
            },
            Item {
                title: "Attach photos",
                detail: "Tap the paperclip to add a photo from your library to your draft. It shows above the message box, and you can remove it before sending.",
            },
            Item {
                title: "Colored code",
                detail: "Code blocks in replies are highlighted so they are easier to read.",
            },
            Item {
                title: "Your draft stays when a message can't send",
                detail: "If a message can't be sent, such as one with a photo while chat takes text only, your text and photo stay in the message box.",
            },
            Item {
                title: "No more \"sending quickly\" warnings",
                detail: "Chat no longer says you're sending messages quickly when you send at a normal pace.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "35",
        title: "Your computer's threads on your phone",
        what_to_test: "With the phone paired to a computer running OpenAgents, open Chats: the computer's threads should be listed, open with their messages, and take a follow-up. While the computer answers, tap stop and the reply should stop. Turn on Airplane Mode: the threads should still open, and a follow-up should say it waits for the computer, then send once when you are back online. Start a Coder run with openagents chat on the computer: it should show in its thread and Stop Coder should stop it. Ask for something that needs a computer: the offer should say which agent will run. In a new chat, \"Who are you\" should still get an answer, and your earlier chats should still be there.",
        items: &[
            Item {
                title: "Your computer's threads are in Chats",
                detail: "Threads from OpenAgents on your paired computer show in Chats. Open one to read it and continue it from the phone.",
            },
            Item {
                title: "Stop a computer's reply",
                detail: "While your computer answers in a thread, tap stop to end the reply. If a Coder run is still going after that, Stop Coder too stops it.",
            },
            Item {
                title: "Threads work offline",
                detail: "Your computer's threads stay on the phone after a relaunch and open without a connection. A follow-up sent offline waits for the computer and sends once when it can.",
            },
            Item {
                title: "Coder runs started on your computer",
                detail: "A Coder run started with openagents chat on your computer shows in its thread on the phone, and you can stop it from there.",
            },
            Item {
                title: "Offers say which agent will run",
                detail: "When a request needs a computer, the offer says which agent and model will run it, or that none is signed in or has room.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "34",
        title: "More chat code shared with desktop",
        what_to_test: "Nothing should look different. Your earlier chats should still be in your chat history and open with their messages. In a new chat, ask \"Who are you\" and a longer question: the replies should stream in, and the suggested questions above the field should not repeat once used. Ask for something that needs a computer: it should offer Connect a computer or Run Coder, not an error.",
        items: &[Item {
            title: "Chat shares more code with the desktop app",
            detail: "The chat screen, your chat history, and the offers under replies now run on the same code as the OpenAgents desktop app. Nothing you see should change, and your earlier chats stay.",
        }],
    },
    Release {
        version: "1.0.0",
        build: "33",
        title: "Shared chat code",
        what_to_test: "Nothing should look different. In a new chat, ask \"Who are you\" and \"How do I connect a phone\": both replies should stream in, and the second should describe scanning the QR code in OpenAgents for Mac. Then close the app fully and open it again: the chat should still be in your chat history.",
        items: &[Item {
            title: "Chat runs on shared code",
            detail: "The chat now runs on the same shared code the desktop app will use. Nothing you see should change.",
        }],
    },
    Release {
        version: "1.0.0",
        build: "32",
        title: "Right answers about connecting",
        what_to_test: "In a new chat, ask \"How do I connect a phone\" and \"Do I need Tailscale?\": the replies should describe scanning the QR code in OpenAgents for Mac (from https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/1.0.0/OpenAgents-1.0.0.dmg), with no Tailscale steps, no commands to run, and no [openagents.…] tags anywhere in the reply. Then connect your Mac: open OpenAgents for Mac, and on the phone tap Account, Computers, Connect a computer and scan the code, or scan it with the iPhone Camera. Both screens should say the computer is connected.",
        items: &[
            Item {
                title: "Answers about connecting a computer",
                detail: "Asking how to connect a phone or a computer now describes the QR code in OpenAgents for Mac, nearby pairing, and copying a code, not the old Tailscale setup.",
            },
            Item {
                title: "No stray tags in replies",
                detail: "Replies drawn from our product notes no longer show ids like [openagents.connect-computer@1].",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "31",
        title: "Connect your Mac",
        what_to_test: "On a Mac, download OpenAgents for Mac from https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/1.0.0/OpenAgents-1.0.0.dmg, open it, drag OpenAgents to Applications, and open it: it shows a QR code. Tick Let this phone open a terminal on this Mac first if you want command cards. On the phone, tap Account, Computers, Connect a computer, and scan the code: both screens should say the computer is connected. On the Mac, choose a project folder and tick Let my phone start Coder here. In a new chat, ask for a change in your project on your Mac and tap Run Coder on your Mac: Coder's steps and reply should show in the chat. Ask who is in the Verse and tap Run on the command card. Then click Remove on the Mac: the phone should show the computer as Revoked. Codex or Claude Code must be signed in on the Mac.",
        items: &[
            Item {
                title: "Connect your Mac",
                detail: "Install OpenAgents for Mac, tap Connect a computer, and scan the code it shows. No Tailscale and no commands to type.",
            },
            Item {
                title: "Run Coder on your Mac from chat",
                detail: "Once your Mac is connected, Run Coder under a reply starts Coder there, and its steps and reply stream into the chat.",
            },
            Item {
                title: "Command cards on your Mac",
                detail: "If you let the phone open a terminal when you scanned, a read-only command card runs on your Mac.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "30",
        title: "Suggestions on every new chat",
        what_to_test: "Open the Chat tab: four suggestions such as Who are you? sit above the box. Tap Who are you? and read the answer. Tap the new-chat button at the top right: Who are you? is gone and another question takes its place. Close the app and open it again: Who are you? is still gone. Type What can you do? yourself, then start a new chat: that one is gone too.",
        items: &[
            Item {
                title: "Suggestions on every new chat",
                detail: "Every new chat shows a few questions to start with, like Who are you? and What's new in the Gym?, not only your very first chat.",
            },
            Item {
                title: "Never the same suggestion twice",
                detail: "Once you tap a suggestion, or type the same question, it doesn't show again, above a new chat or under a reply, even after you close the app. The next one on the list takes its place, and when you've used them all, none show.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "29",
        title: "A simpler chat",
        what_to_test: "Open the Chat tab: there should be no Cloud button at the top and no old chats or Run Coder buttons above the box. Ask \"describe your plugin system\": a spinner and Working… should stay under the reply until it is all there. Then open a long chat from ☰ and scroll it from top to bottom.",
        items: &[
            Item {
                title: "No Cloud button",
                detail: "Every chat goes to OpenAgents, so the Cloud button at the top is gone. When a question needs one of your computers, a Run Coder button shows under our reply.",
            },
            Item {
                title: "Nothing extra above the box",
                detail: "Your previous chats and the Run Coder and Open Coder buttons no longer sit above the message box. Previous chats are behind ☰.",
            },
            Item {
                title: "Previous chats scroll",
                detail: "A chat opened from ☰, or any long chat, scrolls again.",
            },
            Item {
                title: "A working indicator",
                detail: "While we answer, a spinner and Working… show under the reply until it is complete, so a first line is never mistaken for the whole answer.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "28",
        title: "Run Coder without the wall of text",
        what_to_test: "With a computer connected, chat a few turns, then tap Run Coder on it. The task's chat should show one line, \"Continued from the OpenAgents app: \" and the chat's title, not the whole conversation pasted back, and it should scroll as Coder works.",
        items: &[Item {
            title: "One line for a handoff",
            detail: "Running Coder on a computer from a chat used to paste the whole conversation back into the task's chat as one huge message, which could not be scrolled. It now shows one line naming the chat, both while the computer starts and once its transcript arrives.",
        }],
    },
    Release {
        version: "1.0.0",
        build: "27",
        title: "A plain chat",
        what_to_test: "Open the app: the chat header has only the previous-chats button and the OpenAgents title, with no person icon and no Cloud pill, and no line under it. Ask \"Who are you?\": the reply has no \"Prepared answer\" note and no Wrong answer button. Open Account and tap Profile: the Chat tab shows your Profile sheet. Connect a computer and check a Cloud pill appears under the header for choosing where a chat runs.",
        items: &[
            Item {
                title: "A plain chat header",
                detail: "The person icon and the Cloud pill are gone from the chat header, and so is the line under it. Profile is under Account. Where a chat runs shows as a pill under the header only once a computer is connected.",
            },
            Item {
                title: "No notes on replies",
                detail: "Replies no longer say \"Prepared answer\", and the Wrong answer button is gone. Report a problem is still a long press on the tab bar.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "26",
        title: "A fresh chat on upgrade",
        what_to_test: "Install this build over build 24 or 25 without deleting the app. It should open on a fresh chat with OpenAgents, not on the old Project map test. Tap the hamburger, then New chat, and check you get a new chat; the old test chat stays in the list. Everything from build 25 still applies.",
        items: &[
            Item {
                title: "Upgrades open on a fresh chat",
                detail: "A phone whose old guided first run stopped in its test chat opened build 25 on that chat, and New chat put it straight back. The old chat is now history in Previous chats, and the app opens on a new chat.",
            },
            Item {
                title: "New chat is a new chat",
                detail: "New chat and Back always leave the chat you were in. Nothing reopens it on the next frame.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "25",
        title: "Chat first",
        what_to_test: "Delete the app and install this build: it should open on a chat with OpenAgents, with the tab bar, and no Choose Coder screen, step counter, or Gym card. Ask anything. Tap the person icon in the chat header and check Profile opens. Open the Verse, walk into the Gym, open its board, and tap Train Coder: the Chat tab should show Choose Coder, then Let's go, then Start the test. Tap Not now on that first screen and check the chat comes back with no Gym chips; Account, then Train Coder, should bring the intro back.",
        items: &[
            Item {
                title: "You land in chat",
                detail: "A new install opens on a chat with OpenAgents. There is no guided path, no step counter, and no test sent for you. The Gym's starters, cards, and menu wait until you ask for them.",
            },
            Item {
                title: "Train Coder is opt-in",
                detail: "The Gym's intro (Choose Coder, Let's go, Start the test) opens from Train Coder on the Verse's Gym board or under Account. Not now takes you back to chat. What you reached is kept, so a later Train Coder picks up where you left off.",
            },
            Item {
                title: "Profile from the chat",
                detail: "The chat header has a Profile button, and Previous chats is where it was. After your first result, a Menu button leads to the Gym's menu.",
            },
            Item {
                title: "The word is capability",
                detail: "The app says capability where it used to say tool: Test a capability, with and without the capability, Coder has this capability now. Project map, Code finder, and Test reader keep their names.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "24",
        title: "Checks pay either way",
        what_to_test: "From the menu, tap Check a result and read the card: it says a check earns XP whether it confirms the result or not. Run one, add it to the Gym, and check the sheet says the same before and after you publish. Open Profile and check a checked result reads as checked, not confirmed. Ask what it takes for Coder to adopt a capability, and check the answer names a second test set someone else wrote.",
        items: &[
            Item {
                title: "A check earns XP either way",
                detail: "Checking a result pays for the work, not for agreeing. A check that disagrees earns the same XP as one that confirms, stays visible as a disagreement, and counts toward nothing else.",
            },
            Item {
                title: "Adoption needs a second test set",
                detail: "Coder adopts a capability only after three trainers' checks confirm its result and it holds up on a test set someone other than its author wrote. No capability has such a test set yet.",
            },
            Item {
                title: "The numbers beside the verdict",
                detail: "A check's card shows how many tests passed with the capability and without it, the same way the result does, so you can see whether two runs agree on the size of the change and not only on the verdict.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "23",
        title: "Ready for launch",
        what_to_test: "Open Account, then Playtest, and check there is no card of zeros: the playtest card shows only once playtest awards are counted. From the menu, test a tool and check it runs on our computers; if a test set is too big for them, the card says how big it can be. Look through Chat, Wallet, Account, and the Verse and tell us if anything shows a number or a name that isn't yours or the Gym's.",
        items: &[
            Item {
                title: "No sample screens",
                detail: "The offline sample chats, Gym results, wallet, computers, and conversation we used for screenshots are gone from this build. Every screen shows your own records or the Gym's.",
            },
            Item {
                title: "No empty playtest card",
                detail: "Account, then Playtest, no longer shows a card of zeros before playtest awards are counted. The card comes back when they are.",
            },
            Item {
                title: "Plain reasons when we can't run your tests",
                detail: "A card no longer says our test computers aren't open. It says the real reason: a test set bigger than our computers run, with the limit, or a draft this phone no longer has.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "22",
        title: "Clearer credit",
        what_to_test: "From the menu, tap Check a result and run the check; when your XP arrives, tap Check a result again and check the card no longer promises XP for the same test set. Open Profile and check Your results lists only your full runs, and the XP bar fills as you earn. Tap What's new and check the first words show in about a second.",
        items: &[
            Item {
                title: "A second check says what it earns",
                detail: "You earn XP for checking a test set once. A second check of the same test set no longer promises XP, and Profile no longer says XP is on its way for it.",
            },
            Item {
                title: "Results you can check",
                detail: "Check a result offers only results our test computers can run again and that still earn XP when you check them.",
            },
            Item {
                title: "Your results",
                detail: "Profile's Your results lists only your full runs. Tries and checks show under What you made.",
            },
            Item {
                title: "Faster Gym news",
                detail: "What's new in the Gym shows its first words and the news in about a second.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "21",
        title: "Test tools in chat",
        what_to_test: "On a fresh install, tap Choose Coder, then Let's go, then Start the test, and check the test starts in three taps; when the result comes, tap Add to the Gym. From the menu, tap What's new and Check a result, and run a check. In a chat, say \"Help me make a tool that writes changelog entries\" and answer each step with Looks good or a change. Open Profile to see what you made and the XP it earned, and tap The Gym in the Verse to see the board.",
        items: &[
            Item {
                title: "A menu with chat first",
                detail: "The Chat tab opens on a menu whose big button is Chat with OpenAgents, with your trainer name, level, and a line that says what to do next. Test a tool, What's new, and Check a result each start a chat with that question.",
            },
            Item {
                title: "Your first test in three taps",
                detail: "A new install walks you through choosing Coder and testing Project map in chat. If you leave, the app reopens where you were.",
            },
            Item {
                title: "Test a tool from chat",
                detail: "Ask which tool to try and chat shows the tool with Start the test. We run the tests on our computers, with the tool and without it, and the card shows the result: how many tests Coder passed each way.",
            },
            Item {
                title: "Make your own tool by chatting",
                detail: "Chat drafts a tool and its tests with you, one step at a time. Tap Looks good to go on or Change it to say what to change, try it once, then run the full test set. The draft stays on your phone.",
            },
            Item {
                title: "Add to the Gym",
                detail: "Add to the Gym shows exactly what becomes public before anything does. Your result then waits for other trainers to check it.",
            },
            Item {
                title: "Check others' results and earn XP",
                detail: "Check a result runs another trainer's tests again. A check earns XP whether it confirms the result or not, and so does the trainer who added it. Coder adopts a tool only after checks confirm it and it holds up on a test set someone else wrote. Ask what you've earned, or open Profile. XP can't be spent.",
            },
            Item {
                title: "Gym news",
                detail: "Ask what's new in the Gym for the latest results, checks, and builds, each with where it came from.",
            },
            Item {
                title: "The Gym board in the Verse",
                detail: "The Gym in the Verse shows published results by test set and tool, with how many trainers confirmed each. See the board under a result opens it.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "20",
        title: "Smarter chat and a simpler Wallet",
        what_to_test: "In a new chat, ask \"Who are you?\", \"Connect to my GitHub\", \"What's the Grid?\", and \"Fix the failing test in my repo\", and tap the offer under a reply (Run Coder, Connect a computer, or a link to a screen). Open the Wallet tab and try Receive and Send. On a prepared reply, tap Wrong answer.",
        items: &[
            Item {
                title: "Instant answers",
                detail: "Common questions get a prepared answer at once, marked \"Prepared answer\". OpenAgents also answers questions about the app, and about our code with citations to the files it read.",
            },
            Item {
                title: "Work goes to Coder",
                detail: "Ask for work on your code and the reply says what starts, \"Working on…\", with a Run Coder button, or Connect a computer if none is connected yet.",
            },
            Item {
                title: "Links to the right screen",
                detail: "Replies about your wallet, computers, and the rest of the app carry a button that opens that screen.",
            },
            Item {
                title: "Commands on your computer",
                detail: "Some replies offer a read-only command card that runs on your connected computer (one with terminal access) and shows its output in the chat.",
            },
            Item {
                title: "Follow-ups and feedback",
                detail: "Chips under a reply suggest what to ask next. Wrong answer on a reply reports it, and Report a problem can share this chat.",
            },
            Item {
                title: "One voice",
                detail: "Chat speaks as \"we\" everywhere, including the lines about waiting and daily limits.",
            },
            Item {
                title: "A simpler Wallet",
                detail: "The Wallet shows your balance, two big buttons (Receive and Send), and your last five payments. Receive shows a QR code right away; Send takes one pasted or scanned code and figures out what it is. A card asks you to back up your recovery words until you do. Everything else (other ways to receive, buying, deposits, people, agent payments, the amount unit, recovery) is under Advanced.",
            },
            Item {
                title: "Keyboard",
                detail: "Tap outside a text field on any screen to put the keyboard away.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "19",
        title: "New chats go to OpenAgents",
        what_to_test: "With a computer connected, ask \"Who are you?\" in a new chat and check the answer appears within a second.",
        items: &[Item {
            title: "New chats go to OpenAgents",
            detail: "A new chat answers right away even with a computer connected; Coder runs on a computer only when you pick it in the selector or tap a workspace.",
        }],
    },
    Release {
        version: "1.0.0",
        build: "18",
        title: "Chat with OpenAgents",
        what_to_test: "Tap the Chat tab and start typing right away; ask who you're talking to or what model it is, and check the answer is instant. Pick your computer in the selector, send a task, and watch Coder's reply stream in. Open the menu at the top left and check that earlier chats open quickly, and that an OpenCode or Devin session a Coder task started shows inside its chat.",
        items: &[
            Item {
                title: "Ready to type",
                detail: "The Chat tab (the message icon) opens on a new chat, ready to type.",
            },
            Item {
                title: "OpenAgents and Coder",
                detail: "You chat with OpenAgents, which speaks as \"we\"; work for your computer goes to Coder there.",
            },
            Item {
                title: "Where it goes",
                detail: "A selector beside the title picks one of your computers or Cloud, and suggestion chips above the composer continue recent chats or pick a workspace.",
            },
            Item {
                title: "Previous chats",
                detail: "Earlier chats are behind the menu button at the top left. Only OpenAgents and Coder chats are listed; the Claude Code, Codex, OpenCode, and Devin lists are gone.",
            },
            Item {
                title: "Instant answers",
                detail: "Common questions, like who you are talking to or which model answers, get a prepared answer right away.",
            },
            Item {
                title: "Faster chats",
                detail: "Chat lists and transcripts load and open much faster, and Coder on a computer streams its reply.",
            },
            Item {
                title: "Delegated sessions",
                detail: "An OpenCode or Devin session a Coder task delegated to shows inside that Coder chat.",
            },
            Item {
                title: "Wallet notice removed",
                detail: "The wallet no longer shows the \"whole numbers\" notice.",
            },
            Item {
                title: "Lagrange 1 portal hidden",
                detail: "The Grid's portal to Lagrange 1 is hidden for now.",
            },
            Item {
                title: "Playtest logging",
                detail: "On for everyone during the playtest, with no switch: the phone notes which tab and screen you're on, kept on this phone and attached only to a report you preview. The Playtest session toggle is gone.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "17",
        title: "Chat",
        what_to_test: "Open the Chat tab without a computer connected, start a new chat, and watch the basic Coder's reply stream in; then tap Run Coder to hand the conversation to a computer. Check that Devin and OpenCode sessions from your computer show up in the chat list. In Account, Trainer, turn Show my level on and off, and export your trainer card.",
        items: &[
            Item {
                title: "Chat tab",
                detail: "The first tab is Chat. A new chat talks to the basic Coder in the OpenAgents cloud, with no computer needed, and its reply streams in as it's written.",
            },
            Item {
                title: "Run Coder",
                detail: "From a conversation, Run Coder carries it to one of your computers, or Connect a computer leads to Account, Computers.",
            },
            Item {
                title: "Devin and OpenCode chats",
                detail: "The chat list shows Devin CLI and OpenCode sessions from your computers alongside Coder chats.",
            },
            Item {
                title: "Cloud fallback",
                detail: "A computer with no coding agent signed in can still finish a Coder turn through the OpenAgents cloud.",
            },
            Item {
                title: "Trainer level and card",
                detail: "Choose whether your level shows over your head in the Grid, link your keys, and export your trainer card as a signed link.",
            },
            Item {
                title: "Automatic payments",
                detail: "The phone can pay people you trust small amounts without a tap, up to limits you set.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "16",
        title: "Playtesting",
        what_to_test: "Report something from Account, Report a problem, or long-press the tab bar on any screen, and find it in My reports. Turn on Playtest session in Account, Playtest, move around, and check that the log shows only tabs, screens, and times.",
        items: &[
            Item {
                title: "Report a problem",
                detail: "Send a private report with the build and screen filled in, and a screenshot only if you choose one. Never from the Wallet or a key screen.",
            },
            Item {
                title: "Playtest session",
                detail: "An opt-in log of which tab and screen you're on, kept on this phone and attached only to a report you preview.",
            },
            Item {
                title: "My reports",
                detail: "Every report you filed, with the code to quote.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "15",
        title: "Scanning, paying, and long chats",
        what_to_test: "Scan a Lightning invoice or address with the Wallet's scanner and pay a tiny amount. Open a long Coder chat and scroll it. Walk the Grid with the look stick.",
        items: &[
            Item {
                title: "Wallet scanner",
                detail: "The scanner reads payment codes, not only Coder invitations.",
            },
            Item {
                title: "Lightning addresses",
                detail: "Pay Lightning addresses and LNURL codes within the recipient's limits.",
            },
            Item {
                title: "Long chats",
                detail: "Long chats open and scroll faster.",
            },
            Item {
                title: "Look stick",
                detail: "The Grid's look stick turns at a gentler speed.",
            },
        ],
    },
    Release {
        version: "1.0.0",
        build: "1–14",
        title: "First release",
        what_to_test: "Open the app with no help and say what it's for. Walk the Grid and push the ball, open the Gym's RESULTS board, chat with Coder on your own computer, and send a tiny Wallet payment.",
        items: &[
            Item {
                title: "Coder launch",
                detail: "Chat with Coder on your own computers from the Coder tab.",
            },
            Item {
                title: "Verse launch",
                detail: "Walk Verse's world with other players in the Verse tab.",
            },
            Item {
                title: "Computers",
                detail: "Add a computer by invitation or over your tailnet, then order work and open its terminal.",
            },
            Item {
                title: "Identity keys",
                detail: "See this device's npub, and reveal and copy its nsec.",
            },
        ],
    },
];

/// The answer to an `account` request.
#[derive(Serialize)]
pub struct AccountPacket {
    pub schema: &'static str,
    /// The device's public key in NIP-19 form.
    pub npub: String,
    /// The device's public key in hex (x-only, as NIP-01 uses it).
    pub public_hex: String,
    /// How the key was made, for the Identity Keys screen.
    pub origin: &'static str,
    pub changelog: &'static [Release],
    /// The name shown over the player's head in the Verse, cleaned as the
    /// tag draws it; absent until set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// The device's secret key in NIP-19 form. Present only when the
    /// request asked to reveal it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsec: Option<String>,
}

/// What the key is. Both hosts create it from the platform's secure random
/// source and keep it in a this-device-only store.
pub const ORIGIN: &str = "Made at random on this device and kept only in its secure storage. It has no recovery words, so the nsec is its only backup.";

/// The file under the state directory that keeps the display name.
pub const DISPLAY_NAME_FILE: &str = "display-name";

/// The saved display name, cleaned; `None` when unset or unusable.
#[must_use]
pub fn load_display_name(state_dir: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(state_dir.join(DISPLAY_NAME_FILE))
        .ok()
        .and_then(|raw| verse::session::display_name(&raw))
}

/// Saves `name` cleaned, or removes it when nothing drawable is left, and
/// returns what is now saved.
///
/// # Errors
///
/// Returns why the state directory could not be written.
pub fn save_display_name(
    state_dir: &std::path::Path,
    name: &str,
) -> Result<Option<String>, String> {
    let path = state_dir.join(DISPLAY_NAME_FILE);
    match verse::session::display_name(name) {
        Some(cleaned) => {
            std::fs::write(&path, &cleaned).map_err(|e| e.to_string())?;
            Ok(Some(cleaned))
        }
        None => {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
            Ok(None)
        }
    }
}

/// The device's public key as `(hex, npub)`.
pub fn public(secret: &SecretKey) -> (String, String) {
    let (key, _) = secret.x_only_public_key(&Secp256k1::new());
    (key.to_string(), nostr::nip19::encode_npub(&key.serialize()))
}

pub fn packet(secret: &SecretKey, reveal: bool, display_name: Option<String>) -> AccountPacket {
    let (public_hex, npub) = public(secret);
    AccountPacket {
        schema: "openagents.account.v1",
        npub,
        public_hex,
        origin: ORIGIN,
        changelog: CHANGELOG,
        display_name,
        nsec: reveal.then(|| nostr::nip19::encode_nsec(&secret.secret_bytes())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Config, Request};
    use std::str::FromStr;

    // NIP-06's first test vector: the NIP-19 forms of one key pair.
    const SECRET: &str = "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a";
    const PUBLIC: &str = "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917";
    const NPUB: &str = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
    const NSEC: &str = "nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp";

    fn app() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let app = App::new(Config {
            state_dir: dir.path().to_path_buf(),
            secret_hex: SECRET.into(),
        })
        .expect("app");
        (app, dir)
    }

    fn account(app: &mut App, reveal: bool) -> serde_json::Value {
        serde_json::from_slice(&app.respond(Request::Account { reveal })).expect("account packet")
    }

    #[test]
    fn keys_show_as_npub_first_with_hex_beside_it() {
        let secret = SecretKey::from_str(SECRET).expect("secret");
        assert_eq!(public(&secret), (PUBLIC.to_owned(), NPUB.to_owned()));
        let (mut app, _dir) = app();
        let packet = account(&mut app, false);
        assert_eq!(packet["schema"], "openagents.account.v1");
        assert_eq!(packet["npub"], NPUB);
        assert_eq!(packet["public_hex"], PUBLIC);
        assert!(
            packet["origin"]
                .as_str()
                .is_some_and(|o| o.contains("no recovery words"))
        );
        let snapshot: serde_json::Value =
            serde_json::from_slice(&app.respond(Request::Snapshot)).expect("app packet");
        assert_eq!(snapshot["device"], PUBLIC);
        assert_eq!(snapshot["device_npub"], NPUB);
    }

    #[test]
    fn only_an_explicit_reveal_returns_the_nsec() {
        let (mut app, _dir) = app();
        let hidden = account(&mut app, false);
        assert!(hidden.get("nsec").is_none());
        let shown = account(&mut app, true);
        assert_eq!(shown["nsec"], NSEC);
        let decoded = nostr::nip19::decode_nsec(NSEC).expect("nsec");
        assert_eq!(
            decoded,
            SecretKey::from_str(SECRET).expect("secret").secret_bytes()
        );
    }

    #[test]
    fn the_app_packet_never_carries_the_secret_key() {
        let (mut app, _dir) = app();
        let _ = account(&mut app, true);
        let mut packets = vec![
            app.respond(Request::Snapshot),
            app.respond(Request::ComputersRefresh),
        ];
        // Through `call`, which the host never uses for it, a reveal still
        // yields only the app packet.
        packets
            .push(serde_json::to_vec(&app.call(Request::Account { reveal: true })).expect("json"));
        for packet in packets {
            let text = String::from_utf8(packet).expect("utf-8");
            assert!(text.contains(NPUB));
            assert!(!text.contains(SECRET));
            assert!(!text.contains("nsec1"));
        }
    }

    #[test]
    fn build_40_names_the_running_engine_without_usage_limits() {
        let release = CHANGELOG
            .iter()
            .find(|release| release.build == "40")
            .unwrap();
        let detail = release.items[0].detail;
        assert!(detail.contains("the start card names the one running instead"));
        assert!(!detail.contains("limit"));
    }

    #[test]
    fn every_build_has_a_changelog_entry_with_what_to_test() {
        // The newest entry is the build the iOS project makes, or the next
        // one: an entry lands with its change, and the release step bumps
        // the project to it.
        let project = include_str!("../../../bins/openagents-ios/host/project.yml");
        let newest = &CHANGELOG[0];
        let built: u32 = project
            .lines()
            .find_map(|line| line.trim().strip_prefix("CURRENT_PROJECT_VERSION: "))
            .and_then(|build| build.trim().parse().ok())
            .expect("the iOS project's build number");
        let entry: u32 = newest.build.parse().expect("the newest entry's build");
        assert!(
            entry == built || entry == built + 1,
            "the newest Changelog entry is build {entry}; project.yml builds {built}"
        );
        for release in CHANGELOG {
            assert!(!release.what_to_test.trim().is_empty(), "{}", release.build);
            assert!(!release.items.is_empty());
        }
        let first = CHANGELOG.last().expect("first release");
        let titles: Vec<&str> = first.items.iter().map(|item| item.title).collect();
        assert!(titles.contains(&"Coder launch") && titles.contains(&"Verse launch"));
        let (mut app, _dir) = app();
        let packet = account(&mut app, false);
        assert_eq!(packet["changelog"][0]["build"], newest.build);
        assert!(packet["changelog"][0]["what_to_test"].is_string());
    }
}
