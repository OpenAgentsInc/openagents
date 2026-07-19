import type { ReactElement } from "react";
import type { IntentError, IntentReporter, JsonPayload } from "@effect-native/core";
import { ComponentValueBinding, IntentRef } from "@effect-native/core";
import { Effect } from "@effect-native/core/effect";

import { Button } from "#components/ui/button";
import { Input } from "#components/ui/input";
import type { DesktopShellState } from "../shell.ts";

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(
    report(
      payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()),
      payload,
    ) as Effect.Effect<void, IntentError>,
  ).catch(() => undefined);
};

const measured = (value: {
  readonly _tag: string;
  readonly value?: number;
  readonly unit?: string;
}): string =>
  value._tag === "Measured" ? `${value.value ?? 0} ${value.unit ?? ""}`.trim() : "not measured";

export const ReactIdeCursor = ({
  state,
  report,
}: {
  readonly state: DesktopShellState;
  readonly report: IntentReporter;
}): ReactElement => {
  const cursor = state.ideCursor;
  const selected =
    cursor.snapshot.candidates.find(
      (candidate) => candidate.candidateRef === cursor.selectedCandidateRef,
    ) ??
    cursor.snapshot.candidates.at(-1) ??
    null;
  const disclosure = cursor.snapshot.finalDisclosure ?? selected?.disclosure ?? null;
  const accepted =
    selected === null
      ? false
      : cursor.snapshot.receipts.some(
          (receipt) =>
            receipt.applied &&
            receipt.decision._tag === "Accept" &&
            receipt.decision.candidateRef === selected.candidateRef,
        );
  const undone =
    selected === null
      ? false
      : cursor.snapshot.receipts.some(
          (receipt) =>
            receipt.applied &&
            receipt.decision._tag === "Undo" &&
            receipt.decision.candidateRef === selected.candidateRef,
        );
  const proposalSubmitted =
    selected?._tag === "Proposal" &&
    cursor.snapshot.receipts.some(
      (receipt) =>
        receipt.proposalSubmitted &&
        receipt.decision._tag === "Accept" &&
        receipt.decision.candidateRef === selected.candidateRef,
    );
  return (
    <section
      className="oa-react-ide-cursor"
      aria-label="AI editing"
      onKeyDown={(event) => {
        if (event.key !== "Escape" || event.nativeEvent.isComposing || cursor.activeRequest === null) return;
        event.preventDefault();
        dispatch(report, "IdeCursorDecisionRequested", {
          action: "cancel",
          candidateRef: selected?.candidateRef ?? null,
        });
      }}
    >
      <header>
        <strong>AI editing</strong>
        <span data-cursor-state={cursor.snapshot.state}>{cursor.snapshot.state}</span>
        <Button size="sm" variant="ghost" onClick={() => dispatch(report, "IdeCursorRefreshed")}>
          Refresh
        </Button>
      </header>
      <div role="toolbar" aria-label="AI editing requests">
        <Button
          size="sm"
          variant="outline"
          onClick={() => dispatch(report, "IdeCursorCompletionRequested", "all")}
        >
          Complete
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={() => dispatch(report, "IdeCursorNextEditRequested")}
        >
          Next edit
        </Button>
        <Input
          aria-label="Ask or change code"
          maxLength={8_000}
          placeholder="Ask, edit, or generate…"
          value={cursor.prompt}
          onChange={(event) =>
            dispatch(report, "IdeCursorPromptChanged", event.currentTarget.value)
          }
        />
        <Button
          size="sm"
          variant="outline"
          disabled={cursor.prompt.trim() === ""}
          onClick={() => dispatch(report, "IdeCursorAskRequested")}
        >
          Ask
        </Button>
        <Button
          size="sm"
          variant="outline"
          disabled={cursor.prompt.trim() === ""}
          onClick={() => dispatch(report, "IdeCursorEditRequested")}
        >
          Change
        </Button>
        <Button
          size="sm"
          variant="outline"
          disabled={cursor.prompt.trim() === ""}
          onClick={() => dispatch(report, "IdeCursorGenerateRequested")}
        >
          Generate
        </Button>
        {cursor.activeRequest === null ? null : (
          <Button
            size="sm"
            variant="ghost"
            onClick={() =>
              dispatch(report, "IdeCursorDecisionRequested", {
                action: "cancel",
                candidateRef: selected?.candidateRef ?? null,
              })
            }
          >
            Cancel
          </Button>
        )}
      </div>
      {cursor.invalidation === null ? null : (
        <p role="alert">
          Stale {cursor.invalidation._tag.toLocaleLowerCase()}: {cursor.invalidation.detail}
        </p>
      )}
      {cursor.notice === null ? null : (
        <p role="status" aria-live="polite">
          {cursor.notice}
        </p>
      )}
      {cursor.snapshot.failure === null ? null : (
        <p role="alert">
          {cursor.snapshot.failure.reason}: {cursor.snapshot.failure.detail}
        </p>
      )}
      {cursor.snapshot.candidates.length === 0 ? (
        <p>No current AI-editing candidate.</p>
      ) : (
        <div className="oa-react-ide-cursor-candidates">
          <nav aria-label="AI editing candidates">
            {cursor.snapshot.candidates.map((candidate) => (
              <button
                aria-current={candidate.candidateRef === selected?.candidateRef}
                key={candidate.candidateRef}
                onClick={() =>
                  dispatch(report, "IdeCursorCandidateSelected", candidate.candidateRef)
                }
                type="button"
              >
                {candidate._tag} · {Math.round(candidate.quality.confidence * 100)}%
              </button>
            ))}
          </nav>
          {selected === null ? null : (
            <article data-candidate-kind={selected._tag}>
              <header>
                <strong>{selected._tag}</strong>
                <span>
                  {selected.identity.effective.provider.value} /{" "}
                  {selected.identity.effective.model.value}
                </span>
              </header>
              {selected._tag === "Completion" || selected._tag === "NextEdit" ? (
                <pre>{selected.text}</pre>
              ) : selected._tag === "Answer" ? (
                <p>{selected.markdown}</p>
              ) : (
                <p>
                  {selected.proposal.operations.length} file operation(s) require IDE-08 proposal
                  review.
                </p>
              )}
              <div role="toolbar" aria-label="Candidate decisions">
                {selected._tag !== "Completion" && selected._tag !== "NextEdit" ? null : (
                  <>
                    <Button
                      size="sm"
                      disabled={accepted && !undone}
                      onClick={() =>
                        dispatch(report, "IdeCursorDecisionRequested", {
                          action: "accept_word",
                          candidateRef: selected.candidateRef,
                        })
                      }
                    >
                      Accept word
                    </Button>
                    <Button
                      size="sm"
                      disabled={accepted && !undone}
                      onClick={() =>
                        dispatch(report, "IdeCursorDecisionRequested", {
                          action: "accept_line",
                          candidateRef: selected.candidateRef,
                        })
                      }
                    >
                      Accept line
                    </Button>
                    <Button
                      size="sm"
                      disabled={accepted && !undone}
                      onClick={() =>
                        dispatch(report, "IdeCursorDecisionRequested", {
                          action: "accept_all",
                          candidateRef: selected.candidateRef,
                        })
                      }
                    >
                      Accept all
                    </Button>
                  </>
                )}
                {selected._tag !== "Proposal" ? null : (
                  <Button
                    size="sm"
                    disabled={proposalSubmitted}
                    onClick={() =>
                      dispatch(report, "IdeCursorDecisionRequested", {
                        action: "accept_all",
                        candidateRef: selected.candidateRef,
                      })
                    }
                  >
                    Review proposal
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    dispatch(report, "IdeCursorDecisionRequested", {
                      action: "compare",
                      candidateRef: selected.candidateRef,
                    })
                  }
                >
                  Compare
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    dispatch(report, "IdeCursorDecisionRequested", {
                      action: "retry",
                      candidateRef: selected.candidateRef,
                    })
                  }
                >
                  Retry
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() =>
                    dispatch(report, "IdeCursorDecisionRequested", {
                      action: "reject",
                      candidateRef: selected.candidateRef,
                    })
                  }
                >
                  Reject
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={!accepted || undone}
                  onClick={() =>
                    dispatch(report, "IdeCursorDecisionRequested", {
                      action: "undo",
                      candidateRef: selected.candidateRef,
                    })
                  }
                >
                  Undo
                </Button>
              </div>
            </article>
          )}
        </div>
      )}
      {disclosure === null ? null : (
        <details>
          <summary>Provider and data disclosure</summary>
          <p>
            {selected?.identity.effective.harness.value ?? "unknown harness"} ·{" "}
            {selected?.identity.effective.account.value ?? "unknown account"} ·{" "}
            {selected?.identity.effective.placementRef ?? "unknown placement"}
          </p>
          <p>
            Input {measured(disclosure.usage.input)} · Output {measured(disclosure.usage.output)} ·
            Cost {measured(disclosure.usage.cost)}
          </p>
          <p>
            {disclosure.noRemoteIndexDependency
              ? "No remote index dependency"
              : "Remote index dependency disclosed"}{" "}
            · secrets not sent
          </p>
          <ul>
            {disclosure.dataDestinations.map((destination, index) => (
              <li key={`${destination.destination}:${index}`}>
                {destination.destination}: {destination.purpose} ({measured(destination.bytes)},{" "}
                {destination.retention})
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
};
