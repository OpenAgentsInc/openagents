# Mac app integration: handoff state

The actual offscreen application dictation run **passed**: keyboard hold and
mouse hold both inserted the exact public-recording transcript, Undo restored
the original draft, and all three observations recorded zero sends and zero
message parts. The app exited successfully in 20.929 seconds, with no fixture
timeout, truncated output or input identity change.

This executable was generated with an isolated diagnostic compiler using a
2,097,152 work budget. The repository's default 1,048,576 budget is unchanged
and still refuses the full app entry. This runtime result qualifies the
diagnostic executable; the ordinary default-budget build remains blocked,
pending the owner's budget decision or a further compiler correction.

| Evidence | Result and limit |
| --- | --- |
| [Actual app receipt](integration/receipt.json), [output](integration/stdout.txt), [PNG](integration/dictation.png) | Real Parakeet, keyboard, Undo and mouse passed through the actual offscreen event loop. The transcript became an editable draft and was never sent. |
| [Screen negative profile v4](dictation-screen-negative-v4.stdout) | 20/20 selected checks passed, including terminal cancellation, focus restoration, a success waiting for draft insertion, literal text, Undo, no send, and caller-owned OpenRouter headers. This is Coax execution, not a launched application. |
| [Reused native inputs](reused-inputs.json) | Hashes of the unchanged chat/router/process/shader inputs and separately qualified controller and Parakeet archive. No repeated model corpus or unchanged mutation gate. |
| [App-check tool proof](tool/record.json) | Four oracle groups passed. Native tool generation and usage/invalid-input refusals passed. No app process was started for those checks. |
| [Isolated 2M diagnostic build](diagnostic-build/record.json) | Generated app C/header are byte-identical to the compiled native inputs. Derived compiler/app C and headers, compile/link diagnostics and producer hashes are retained. This does not qualify an ordinary default-budget build. |
| [Default-budget generation refusal](settings-final.stderr) | The 1,048,576-budget attempt reports E0381 at `coax/lib/systemone/request.coax:169`; generated app C was empty. The isolated 2M generation succeeds. Earlier retained-borrow and compiler attempts remain as historical observations. |
| [Handoff inventory](handoff-inventory.json) | Read-only hashes of the current changed sources, existing component receipts and available native inputs. A dated working-tree snapshot; it does not upgrade any component's historical qualification or certify a completed app build. |

The retained run used
[`check-dictation-app.coax`](../../../../apps/openagents-desktop/tests/check-dictation-app.coax)
against that exact executable. The tool uses the public Parakeet recording,
actual keyboard and mouse handlers, exact composer text, Undo, and three
observations of `sent=0 parts=0`. It saves the script, output, screenshot and
hash-pinned receipt, and refuses timeouts, truncated logs, changed inputs and
an old screenshot. Its app process is capped at three minutes.

The synthetic 500 ms silent interval between Undo and the mouse gesture
represents new idle microphone samples in the fixture clock; it prevents the
first utterance's tail remaining in the second gesture's pre-roll. This is a
correctness check, not a release-latency measurement. Capture is injected:
there is no microphone permission request, private recording, visible window
or audio playback.

The [aggregate record](record.json) pins the signed executable, model archive,
build diagnostics and integration artifacts. The [inventory](handoff-inventory.json)
records the current source snapshot without rewriting older component source
pins. Owner microphone and macOS permission/focus acceptance remain in
[`NEEDS_OWNER.md`](../../../../NEEDS_OWNER.md).
