// Kiki's house editor desk (`.kiki` pages only). It mounts beside the studio
// and editor controllers (`../aethyr/house-host.tsx`) and changes nothing they
// draw beyond presentation:
//
// - Cards. The editor's Jarvis sheet ("tools") and the voice bar ("voice")
//   are Aethyr panels; the desk marks them `data-kiki-pane` and gives each a
//   header to drag it by and an × to close it. Chat, World inventory, Edits
//   and payouts, and World actions are the desk's own cards.
// - Commands. "show inventory", "close chat" (typed to the agent, or heard by
//   the house's voice controls) open and close cards (`commands.ts`). Any
//   other message to the agent goes to Kiln through the release's own
//   request field and Review request button.
// - Guest practice. A guest keeps one change at a time; a second attempt
//   cancels through the release's Cancel and asks for a revert.
//
// Every house action is a click on, or a value for, the release's own
// control by its ID, exactly as the editor controller sends them.
import { createRoot } from "react-dom/client";
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import * as Atom from "effect/reactivity/Atom";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import { RegistryContext, useAtom, useAtomValue } from "@effect/atom-react";
import { CARD_NAMES, parseCommand, type Card, type Command } from "./commands";
import {
  append,
  apply,
  blocksChange,
  clamp,
  clock,
  initialDesk,
  isOpen,
  move,
  payoutLine,
  restore,
  revertControl,
  type DeskState,
  type Message,
  type Point,
  type Room,
} from "./desk";

const byId = (id: string) => document.getElementById(id);
const textOf = (node: Element | null | undefined) =>
  (node?.textContent ?? "").replace(/\s+/g, " ").trim();
const STORE = "kiki-desk-v1";
const PHONE = 800;

/** The page carries `kiki` on `<html>` or `<body>`. */
export const isKiki = (doc: Document = document) =>
  doc.documentElement.classList.contains("kiki") ||
  (doc.body?.classList.contains("kiki") ?? false);

/** A flag the page sets on `<html>` or `<body>`, as `data-<name>`. */
const flag = (name: string): string | undefined =>
  document.body?.dataset[name] ?? document.documentElement.dataset[name];

// --- What the house shows -----------------------------------------------

interface Offer {
  readonly shown: boolean;
  readonly enabled: boolean;
}

/** One release control, by its own `hidden` and `disabled`. */
const offer = (id: string): Offer => {
  const node = byId(id) as HTMLButtonElement | null;
  return {
    shown: node !== null && !node.hidden,
    enabled: node !== null && !node.hidden && !node.disabled,
  };
};

export interface HouseView {
  readonly editing: boolean;
  readonly place: boolean;
  readonly at: string;
  readonly status: string;
  readonly kiln: string;
  readonly heard: string;
  readonly request: boolean;
  readonly visit: ReadonlyArray<string>;
  readonly saved: ReadonlyArray<ReadonlyArray<string>>;
  readonly savedShown: boolean;
  readonly savedStatus: string;
  readonly refresh: boolean;
  readonly earnings: string;
  readonly play: boolean;
  readonly compile: boolean;
  readonly revert: boolean;
  readonly publish: boolean;
}

const items = (id: string) =>
  Array.from(byId(id)?.children ?? [], (li) => textOf(li)).filter(Boolean);

function readHouse(): HouseView {
  const placeButton = byId("edit-place") as HTMLButtonElement | null;
  const coordinate = (id: string) =>
    (byId(id) as HTMLInputElement | null)?.value ?? "";
  const saved = byId("contribution-tools");
  return {
    editing:
      document.querySelector('#house-editor [data-aethyr-id="edit-panel"]') !==
      null,
    place:
      placeButton !== null &&
      !placeButton.disabled &&
      placeButton.closest("[hidden]") === null,
    at: ["edit-x", "edit-y", "edit-z"].map(coordinate).join(", "),
    status: textOf(byId("edit-status")),
    kiln: textOf(byId("edit-kiln-status")),
    heard: textOf(byId("voice-heard")),
    request:
      offer("edit-request-review").enabled &&
      byId("edit-kiln-request") !== null,
    visit: items("edit-history-entries"),
    saved: Array.from(byId("project-history-rows")?.children ?? [], (li) => {
      const texts = Array.from(li.children)
        .filter((child) => !child.matches("button, a"))
        .map((child) => textOf(child))
        .filter(Boolean);
      return texts.length > 0 || li.children.length > 0 ? texts : [textOf(li)];
    }).filter((texts) => texts.length > 0),
    savedShown: saved !== null && saved.closest("[hidden]") === null,
    savedStatus: textOf(byId("project-history-status")),
    refresh: offer("project-history-refresh").enabled,
    earnings: textOf(byId("visit-earnings")),
    play: offer("project-play").enabled,
    compile: offer("request-compile").enabled,
    revert: offer("project-revert").enabled,
    publish: offer("project-publish").enabled,
  };
}

const EMPTY_HOUSE: HouseView = {
  editing: false,
  place: false,
  at: "",
  status: "",
  kiln: "",
  heard: "",
  request: false,
  visit: [],
  saved: [],
  savedShown: false,
  savedStatus: "",
  refresh: false,
  earnings: "",
  play: false,
  compile: false,
  revert: false,
  publish: false,
};

/** Click the release's own control. */
const press = (id: string) => byId(id)?.click();

/** Give the release's own field a value, as the editor controller does. */
function fill(id: string, value: string) {
  const field = byId(id) as HTMLInputElement | null;
  if (!field) return false;
  field.value = value;
  field.dispatchEvent(new Event("input", { bubbles: true }));
  field.dispatchEvent(new Event("change", { bubbles: true }));
  return true;
}

interface Asset {
  readonly id: string;
  readonly name: string;
}

function readAssets(): ReadonlyArray<Asset> {
  try {
    const value = JSON.parse(byId("creator-assets")?.textContent ?? "[]");
    return Array.isArray(value)
      ? value.filter(
          (a): a is Asset =>
            typeof a?.id === "string" && typeof a?.name === "string",
        )
      : [];
  } catch {
    return [];
  }
}

// --- State ---------------------------------------------------------------

const save = (state: DeskState) => {
  try {
    localStorage.setItem(STORE, JSON.stringify(state));
  } catch {
    // Private windows and blocked storage keep the desk for this visit only.
  }
};
const load = (fallback: DeskState) => {
  try {
    return restore(localStorage.getItem(STORE), fallback);
  } catch {
    return fallback;
  }
};

interface Practice {
  readonly revert: "edit-undo" | "project-revert" | undefined;
}

const deskAtom = Atom.make<DeskState>(initialDesk(false));
const houseAtom = Atom.make<HouseView>(EMPTY_HOUSE);
const roomAtom = Atom.make<Room>("agent");
const agentLogAtom = Atom.make<ReadonlyArray<Message>>([]);
const worldLogAtom = Atom.make<ReadonlyArray<Message>>([]);
const peopleAtom = Atom.make<ReadonlyArray<string> | undefined>(undefined);
const worldOpenAtom = Atom.make(false);
const practiceAtom = Atom.make<Practice | undefined>(undefined);
const noticeAtom = Atom.make("");

let serial = 0;
const message = (name: string, text: string, mine: boolean): Message => ({
  id: `m${Date.now().toString(36)}-${serial++}`,
  name,
  text,
  at: Date.now(),
  mine,
});

// --- Panes ---------------------------------------------------------------

/** The Aethyr panels the desk makes into cards. */
const AETHYR_PANES: ReadonlyArray<readonly [Card, string]> = [
  ["tools", '#house-editor [data-aethyr-id="edit-panel"]'],
  ["voice", '#house-hud [data-aethyr-id="world-voice"]'],
];

function bar(card: Card): HTMLElement {
  const element = document.createElement("div");
  element.className = "kiki-pane-bar";
  element.dataset.kikiBar = card;
  element.tabIndex = 0;
  element.setAttribute("role", "group");
  element.setAttribute(
    "aria-label",
    `${CARD_NAMES[card]}: drag, or use the arrow keys, to move`,
  );
  const title = document.createElement("span");
  title.className = "kiki-pane-title";
  title.textContent = CARD_NAMES[card];
  const close = document.createElement("button");
  close.type = "button";
  close.className = "kiki-pane-close";
  close.dataset.kikiClose = card;
  close.setAttribute("aria-label", `Close ${CARD_NAMES[card]}`);
  close.textContent = "×";
  element.append(title, close);
  return element;
}

function setAttr(node: HTMLElement, name: string, value: string | undefined) {
  if (value === undefined) {
    if (node.hasAttribute(name)) node.removeAttribute(name);
  } else if (node.getAttribute(name) !== value) node.setAttribute(name, value);
}
function setStyle(node: HTMLElement, name: string, value: string) {
  if (node.style.getPropertyValue(name) !== value)
    node.style.setProperty(name, value);
}

/** Bring every card on the page in line with the desk. */
function syncPanes(state: DeskState) {
  for (const [card, selector] of AETHYR_PANES) {
    const node = document.querySelector<HTMLElement>(selector);
    if (!node) continue;
    setAttr(node, "data-kiki-pane", card);
    if (!node.querySelector(`:scope > [data-kiki-bar="${card}"]`))
      node.prepend(bar(card));
  }
  for (const node of document.querySelectorAll<HTMLElement>(
    "[data-kiki-pane]",
  )) {
    const card = node.dataset.kikiPane as Card;
    setAttr(node, "data-kiki-closed", isOpen(state, card) ? undefined : "");
    const at = state.moved[card];
    setAttr(node, "data-kiki-moved", at ? "" : undefined);
    setStyle(node, "left", at ? `${at.x}px` : "");
    setStyle(node, "top", at ? `${at.y}px` : "");
    setStyle(node, "--kiki-w", at?.w ? `${at.w}px` : "");
  }
}

// --- Cards ---------------------------------------------------------------

function Bar({ card }: { card: Card }) {
  return (
    <div
      className="kiki-pane-bar"
      data-kiki-bar={card}
      tabIndex={0}
      role="group"
      aria-label={`${CARD_NAMES[card]}: drag, or use the arrow keys, to move`}
    >
      <span className="kiki-pane-title">{CARD_NAMES[card]}</span>
      <button
        type="button"
        className="kiki-pane-close"
        data-kiki-close={card}
        aria-label={`Close ${CARD_NAMES[card]}`}
      >
        ×
      </button>
    </div>
  );
}

function Pane({ card, children }: { card: Card; children: ReactNode }) {
  return (
    <section
      className="kiki-card"
      data-kiki-pane={card}
      aria-label={CARD_NAMES[card]}
    >
      <Bar card={card} />
      <div className="kiki-card-body">{children}</div>
    </section>
  );
}

function Messages({
  log,
  empty,
}: {
  log: ReadonlyArray<Message>;
  empty: string;
}) {
  const end = useRef<HTMLLIElement>(null);
  useLayoutEffect(() => {
    end.current?.scrollIntoView?.({ block: "nearest" });
  }, [log.length]);
  if (log.length === 0) return <p className="kiki-empty">{empty}</p>;
  return (
    <ol className="kiki-msgs" aria-live="polite">
      {log.map((m, index) => (
        <li
          key={m.id}
          ref={index === log.length - 1 ? end : undefined}
          className={m.mine ? "kiki-msg kiki-mine" : "kiki-msg"}
        >
          <span className="kiki-who">
            {m.name}
            <time dateTime={new Date(m.at).toISOString()}>{clock(m.at)}</time>
          </span>
          <span className="kiki-txt">{m.text}</span>
        </li>
      ))}
    </ol>
  );
}

function Composer({
  label,
  enabled,
  onSend,
}: {
  label: string;
  enabled: boolean;
  onSend: (text: string) => void;
}) {
  const [text, setText] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const value = text.trim();
    if (!value || !enabled) return;
    setText("");
    onSend(value);
  };
  return (
    <form className="kiki-composer" onSubmit={submit}>
      <input
        type="text"
        aria-label={label}
        placeholder={label}
        maxLength={1200}
        value={text}
        disabled={!enabled}
        onChange={(event) => setText(event.currentTarget.value)}
        onKeyDown={(event) => event.stopPropagation()}
      />
      <button type="submit" className="kiki-button" disabled={!enabled}>
        Send
      </button>
    </form>
  );
}

function ChatCard({ onAgent }: { onAgent: (text: string) => void }) {
  const [room, setRoom] = useAtom(roomAtom);
  const agent = useAtomValue(agentLogAtom);
  const world = useAtomValue(worldLogAtom);
  const people = useAtomValue(peopleAtom);
  const worldOpen = useAtomValue(worldOpenAtom);
  const [, setWorld] = useAtom(worldLogAtom);
  const tab = (id: Room, label: string) => (
    <button
      type="button"
      role="tab"
      id={`kiki-tab-${id}`}
      aria-selected={room === id}
      aria-controls="kiki-room"
      className={room === id ? "kiki-tab kiki-on" : "kiki-tab"}
      onClick={() => setRoom(id)}
    >
      {label}
    </button>
  );
  return (
    <Pane card="chat">
      <div className="kiki-tabs" role="tablist" aria-label="Chat rooms">
        {tab("world", "World")}
        {tab("agent", "Agent")}
      </div>
      <div
        id="kiki-room"
        role="tabpanel"
        aria-labelledby={`kiki-tab-${room}`}
        className="kiki-room"
      >
        {room === "world" ? (
          <>
            {people && (
              <p className="kiki-people">
                {people.length === 0
                  ? "Only you are here."
                  : `Here now: ${people.join(", ")}`}
              </p>
            )}
            <Messages
              log={world}
              empty={
                worldOpen
                  ? "No messages yet. Say hello."
                  : "World chat isn't open in this world yet."
              }
            />
            <Composer
              label="Message people in this world"
              enabled={worldOpen}
              onSend={(text) => {
                setWorld((log) => append(log, message("You", text, true)));
                document.dispatchEvent(
                  new CustomEvent("kiki:world-send", { detail: { text } }),
                );
              }}
            />
          </>
        ) : (
          <>
            <Messages
              log={agent}
              empty='Ask for a change, or say "show inventory" or "close chat".'
            />
            <Composer label="Message the agent" enabled onSend={onAgent} />
          </>
        )}
      </div>
    </Pane>
  );
}

function InventoryCard({ assets }: { assets: ReadonlyArray<Asset> }) {
  const house = useAtomValue(houseAtom);
  const [query, setQuery] = useState("");
  const [placed, setPlaced] = useState("");
  const shown = assets.filter((a) =>
    a.name.toLowerCase().includes(query.trim().toLowerCase()),
  );
  return (
    <Pane card="inventory">
      <input
        type="search"
        className="kiki-search"
        aria-label="Find an asset"
        placeholder="Find an asset"
        value={query}
        onChange={(event) => setQuery(event.currentTarget.value)}
        onKeyDown={(event) => event.stopPropagation()}
      />
      <p className="kiki-caption">
        {house.place
          ? `Adds at ${house.at} (X, Y, Z in Object tools).`
          : "Props can't be added here right now."}
      </p>
      {assets.length === 0 ? (
        <p className="kiki-empty">This world has no assets to add.</p>
      ) : (
        <ul className="kiki-assets" aria-label="Assets">
          {shown.map((asset) => (
            <li key={asset.id}>
              <span>{asset.name}</span>
              <button
                type="button"
                className="kiki-button"
                disabled={!house.place}
                aria-label={`Add ${asset.name}`}
                onClick={() => {
                  if (!fill("edit-place-asset", asset.id)) return;
                  press("edit-place");
                  setPlaced(asset.name);
                }}
              >
                Add
              </button>
            </li>
          ))}
        </ul>
      )}
      {placed && (
        <p className="kiki-caption" role="status">
          {house.status || `Adding ${placed}…`}
        </p>
      )}
    </Pane>
  );
}

function PayoutsCard() {
  const house = useAtomValue(houseAtom);
  return (
    <Pane card="payouts">
      <h3 className="kiki-heading">This visit</h3>
      {house.visit.length === 0 ? (
        <p className="kiki-empty">No changes yet.</p>
      ) : (
        <ol className="kiki-log">
          {house.visit.map((line, index) => (
            <li key={index}>{line}</li>
          ))}
        </ol>
      )}
      {house.earnings && (
        <p className="kiki-amount">{payoutLine(house.earnings)}</p>
      )}
      {house.savedShown && (
        <>
          <h3 className="kiki-heading">Saved changes</h3>
          {house.savedStatus && (
            <p className="kiki-caption" role="status">
              {house.savedStatus}
            </p>
          )}
          {house.saved.length > 0 && (
            <ol className="kiki-log">
              {house.saved.map((lines, index) => (
                <li key={index}>
                  {lines.map((line, n) => (
                    <span key={n} className={n === 0 ? "" : "kiki-caption"}>
                      {payoutLine(line)}
                    </span>
                  ))}
                </li>
              ))}
            </ol>
          )}
          {house.refresh && (
            <button
              type="button"
              className="kiki-button"
              onClick={() => press("project-history-refresh")}
            >
              Refresh
            </button>
          )}
        </>
      )}
    </Pane>
  );
}

const ACTIONS = [
  ["play", "project-play", "Play"],
  ["compile", "request-compile", "Compile"],
  ["revert", "project-revert", "Revert"],
  ["publish", "project-publish", "Publish"],
] as const;

function ActionsCard() {
  const house = useAtomValue(houseAtom);
  return (
    <Pane card="actions">
      <div className="kiki-row">
        {ACTIONS.map(([key, id, label]) => (
          <button
            key={key}
            type="button"
            className={
              key === "publish" ? "kiki-button kiki-solid" : "kiki-button"
            }
            disabled={!house[key]}
            onClick={() => press(id)}
          >
            {label}
          </button>
        ))}
      </div>
    </Pane>
  );
}

function PracticeBox({ onDone }: { onDone: (reverted: boolean) => void }) {
  const practice = useAtomValue(practiceAtom);
  const revert = useRef<HTMLButtonElement>(null);
  const keep = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (practice) (revert.current ?? keep.current)?.focus();
  }, [practice]);
  if (!practice) return null;
  return (
    <div
      className="kiki-overlay"
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === "Escape") onDone(false);
      }}
    >
      <div
        className="kiki-practice"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="kiki-practice-title"
        aria-describedby="kiki-practice-text"
      >
        <h2 id="kiki-practice-title">Revert to make another change</h2>
        <p id="kiki-practice-text">
          Guests keep one change at a time. Revert your change to try another,
          or keep the one you made.
        </p>
        <div className="kiki-row kiki-center">
          <button
            ref={revert}
            type="button"
            className="kiki-button kiki-ember"
            disabled={!practice.revert}
            onClick={() => {
              if (practice.revert) press(practice.revert);
              onDone(true);
            }}
          >
            Revert
          </button>
          <button
            ref={keep}
            type="button"
            className="kiki-button"
            onClick={() => onDone(false)}
          >
            Keep this change
          </button>
        </div>
      </div>
    </div>
  );
}

function Desk({
  assets,
  onAgent,
  onPractice,
}: {
  assets: ReadonlyArray<Asset>;
  onAgent: (text: string) => void;
  onPractice: (reverted: boolean) => void;
}) {
  const house = useAtomValue(houseAtom);
  const [desk, setDesk] = useAtom(deskAtom);
  const notice = useAtomValue(noticeAtom);
  return (
    <>
      {house.editing && (
        <>
          {isOpen(desk, "actions") && <ActionsCard />}
          {isOpen(desk, "inventory") && <InventoryCard assets={assets} />}
          {isOpen(desk, "chat") && <ChatCard onAgent={onAgent} />}
          {isOpen(desk, "payouts") && <PayoutsCard />}
          {!isOpen(desk, "chat") && (
            <button
              type="button"
              className="kiki-reopen"
              onClick={() =>
                setDesk(
                  apply(
                    desk,
                    { verb: "open", card: "chat" },
                    window.innerWidth < PHONE,
                  ),
                )
              }
            >
              Chat with the agent
            </button>
          )}
        </>
      )}
      {notice && (
        <p className="kiki-notice" role="status">
          {notice}
        </p>
      )}
      <PracticeBox onDone={onPractice} />
    </>
  );
}

// --- Mount ---------------------------------------------------------------

async function signedIn(): Promise<boolean | undefined> {
  try {
    const response = await fetch("/v1/verse/studio/session", {
      credentials: "same-origin",
      cache: "no-store",
    });
    if (!response.ok) return undefined;
    const value = (await response.json()) as { signed_in?: unknown };
    return value.signed_in === true;
  } catch {
    return undefined;
  }
}

export function mountKiki(): () => void {
  const registry = AtomRegistry.make();
  // The world has characters once one speaks (host protocol v8).
  let characters = false;
  const phone = window.innerWidth < PHONE;
  registry.set(deskAtom, load(initialDesk(phone)));
  const assets = readAssets();
  const element = document.createElement("div");
  element.id = "kiki-desk";
  document.body.append(element);

  // A guest unless the page says otherwise or the visitor is signed in.
  let session: boolean | undefined;
  const refreshSession = () =>
    void signedIn().then((value) => {
      if (value !== undefined) session = value;
    });
  refreshSession();
  const guest = () => {
    const stated = flag("kikiGuest");
    if (stated === "true") return true;
    if (stated === "false") return false;
    return session === false;
  };

  const desk = () => registry.get(deskAtom);
  const setDesk = (next: DeskState) => {
    registry.set(deskAtom, next);
    save(next);
    syncPanes(next);
  };
  let noticeTimer = 0;
  const say = (text: string) => {
    registry.set(noticeAtom, text);
    window.clearTimeout(noticeTimer);
    noticeTimer = window.setTimeout(() => registry.set(noticeAtom, ""), 4000);
  };

  /** Open or close a card; say what happened, as the agent. */
  const run = (command: Command): string => {
    const name = CARD_NAMES[command.card];
    if (command.verb === "open" && !registry.get(houseAtom).editing) {
      // Every card lives in the editor; opening one opens the editor.
      press("world-edit");
    }
    setDesk(apply(desk(), command, window.innerWidth < PHONE));
    return command.verb === "open" ? `Opened ${name}.` : `Closed ${name}.`;
  };

  const agentSay = (text: string) =>
    registry.update(agentLogAtom, (log) =>
      append(log, message("Agent", text, false)),
    );
  const onAgent = (text: string) => {
    registry.update(agentLogAtom, (log) =>
      append(log, message("You", text, true)),
    );
    const command = parseCommand(text);
    if (command) return agentSay(run(command));
    if (!registry.get(houseAtom).request || !fill("edit-kiln-request", text))
      return agentSay("You can't send requests in this world right now.");
    press("edit-request-review");
    agentSay("I sent your request to Kiln. Its answer will show here.");
  };

  const onPractice = (reverted: boolean) => {
    registry.set(practiceAtom, undefined);
    if (reverted) say("Change reverted. Make your next change.");
  };

  const root = createRoot(element);
  root.render(
    <RegistryContext.Provider value={registry}>
      <Desk assets={assets} onAgent={onAgent} onPractice={onPractice} />
    </RegistryContext.Provider>,
  );

  // Follow the house.
  let heard = "";
  let kiln = readHouse().kiln;
  const poll = () => {
    const next = readHouse();
    const before = registry.get(houseAtom);
    if (JSON.stringify(next) !== JSON.stringify(before))
      registry.set(houseAtom, next);
    document.documentElement.toggleAttribute("data-kiki-editing", next.editing);
    if (next.kiln && next.kiln !== kiln)
      registry.update(agentLogAtom, (log) =>
        append(log, message("Kiln", next.kiln, false)),
      );
    kiln = next.kiln;
    // A card command heard by the house's voice controls.
    if (next.heard && next.heard !== heard) {
      const spoken = next.heard.replace(/^[^:]{0,24}:\s*/, "");
      const command = parseCommand(next.heard) ?? parseCommand(spoken);
      if (command) say(run(command));
    }
    heard = next.heard;
    registry.set(worldOpenAtom, flag("kikiWorldChat") === "open" || characters);
  };

  // The guest rule: watch the release's change confirmation open.
  let confirming = false;
  const watchAuth = () => {
    const dialog = byId("edit-auth") as HTMLDialogElement | null;
    const open = dialog !== null && dialog.open && !dialog.hidden;
    if (open && !confirming) {
      const attempt = {
        guest: guest(),
        changes: items("edit-history-entries").length,
        saving: textOf(byId("edit-auth-title")) === "Save this world?",
      };
      refreshSession();
      if (blocksChange(attempt)) {
        press("edit-cancel");
        registry.set(practiceAtom, {
          revert: revertControl({
            undo: offer("edit-undo"),
            revert: offer("project-revert"),
          }),
        });
      }
    }
    confirming = open;
  };

  let frame = 0;
  const schedule = () => {
    if (!frame)
      frame = requestAnimationFrame(() => {
        frame = 0;
        syncPanes(desk());
      });
  };
  const observer = new MutationObserver((records) => {
    if (
      records.some(
        (r) => r.target instanceof Element && r.target.id === "edit-auth",
      )
    )
      watchAuth();
    if (records.some((r) => r.type === "childList")) schedule();
  });
  observer.observe(document.body, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ["open", "hidden"],
  });
  const unsubscribe = registry.subscribe(deskAtom, schedule);
  const timer = window.setInterval(() => {
    poll();
    watchAuth();
  }, 300);
  poll();
  schedule();

  // Cards: close, drag, and move with the keyboard.
  const cardOf = (target: EventTarget | null) => {
    const node = target instanceof Element ? target : null;
    const pane = node?.closest<HTMLElement>("[data-kiki-pane]");
    return pane ? { pane, card: pane.dataset.kikiPane as Card } : undefined;
  };
  const view = () => ({ width: window.innerWidth, height: window.innerHeight });
  const place = (card: Card, pane: HTMLElement, to: Point) => {
    const at = clamp(to, pane.getBoundingClientRect(), view());
    const w = Math.round(pane.getBoundingClientRect().width);
    return { ...at, w };
  };
  const click = (event: MouseEvent) => {
    const close = (event.target as Element | null)?.closest?.<HTMLElement>(
      "[data-kiki-close]",
    );
    if (!close) return;
    event.preventDefault();
    event.stopPropagation();
    setDesk(
      apply(desk(), { verb: "close", card: close.dataset.kikiClose as Card }),
    );
  };
  let drag:
    | { card: Card; pane: HTMLElement; dx: number; dy: number; id: number }
    | undefined;
  const down = (event: PointerEvent) => {
    const target = event.target as Element | null;
    const grip = target?.closest?.("[data-kiki-bar]");
    if (!grip || target?.closest("button") || event.button !== 0) return;
    const found = cardOf(grip);
    if (!found) return;
    const box = found.pane.getBoundingClientRect();
    drag = {
      ...found,
      dx: event.clientX - box.left,
      dy: event.clientY - box.top,
      id: event.pointerId,
    };
    (grip as HTMLElement).setPointerCapture?.(event.pointerId);
    event.preventDefault();
  };
  const moveTo = (event: PointerEvent) => {
    if (!drag || event.pointerId !== drag.id) return;
    const at = place(drag.card, drag.pane, {
      x: event.clientX - drag.dx,
      y: event.clientY - drag.dy,
    });
    registry.set(deskAtom, move(desk(), drag.card, at));
    syncPanes(desk());
  };
  const up = (event: PointerEvent) => {
    if (!drag || event.pointerId !== drag.id) return;
    drag = undefined;
    save(desk());
  };
  const key = (event: KeyboardEvent) => {
    const grip = (event.target as Element | null)?.closest?.("[data-kiki-bar]");
    if (!grip || event.target !== grip) return;
    const step = event.shiftKey ? 64 : 16;
    const delta: Record<string, [number, number]> = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, -step],
      ArrowDown: [0, step],
    };
    const d = delta[event.key];
    const found = cardOf(grip);
    if (!d || !found) return;
    event.preventDefault();
    event.stopPropagation();
    const box = found.pane.getBoundingClientRect();
    setDesk(
      move(
        desk(),
        found.card,
        place(found.card, found.pane, {
          x: box.left + d[0],
          y: box.top + d[1],
        }),
      ),
    );
  };
  // Panes the visitor can no longer reach come back into view.
  const resize = () => {
    const state = desk();
    let next = state;
    for (const node of document.querySelectorAll<HTMLElement>(
      "[data-kiki-pane]",
    )) {
      const card = node.dataset.kikiPane as Card;
      const at = state.moved[card];
      if (!at) continue;
      const fixed = {
        ...clamp(at, node.getBoundingClientRect(), view()),
        w: at.w,
      };
      if (fixed.x !== at.x || fixed.y !== at.y) next = move(next, card, fixed);
    }
    if (next !== state) setDesk(next);
  };
  // A character's line (`aethyr-npc`, host protocol v8) joins World chat;
  // a World message naming one (`@ghost hello`) asks it to reply.
  const npcLine = (event: Event) => {
    const line = (event as CustomEvent).detail as
      { name?: unknown; text?: unknown; kind?: unknown } | undefined;
    if (typeof line?.name !== "string" || typeof line.text !== "string") return;
    characters = true;
    registry.set(worldOpenAtom, true);
    const text = line.kind === "emote" ? `* ${line.text} *` : line.text;
    registry.update(worldLogAtom, (log) =>
      append(log, message(line.name as string, text, false)),
    );
  };
  const worldSend = (event: Event) => {
    const text = (event as CustomEvent).detail?.text;
    const demo = (
      window as { hauntDemo?: { npc_chat?: (text: string) => unknown } }
    ).hauntDemo;
    if (typeof text !== "string" || !/^\s*@/.test(text)) return;
    if (typeof demo?.npc_chat !== "function") return;
    try {
      demo.npc_chat(text.trim());
    } catch (error) {
      const why =
        typeof error === "string" ? error : "No one here by that name.";
      registry.update(worldLogAtom, (log) =>
        append(log, message("World", why, false)),
      );
    }
  };
  const worldMessage = (event: Event) => {
    const detail = (event as CustomEvent).detail as
      { name?: unknown; text?: unknown; at?: unknown } | undefined;
    if (typeof detail?.name !== "string" || typeof detail.text !== "string")
      return;
    registry.update(worldLogAtom, (log) =>
      append(log, {
        ...message(detail.name as string, detail.text as string, false),
        ...(typeof detail.at === "number" && { at: detail.at }),
      }),
    );
  };
  const worldPeople = (event: Event) => {
    const names = (event as CustomEvent).detail?.names;
    if (Array.isArray(names))
      registry.set(
        peopleAtom,
        names.filter((n): n is string => typeof n === "string"),
      );
  };
  document.addEventListener("click", click, true);
  document.addEventListener("pointerdown", down);
  document.addEventListener("pointermove", moveTo);
  document.addEventListener("pointerup", up);
  document.addEventListener("pointercancel", up);
  document.addEventListener("keydown", key, true);
  document.addEventListener("kiki:world-message", worldMessage);
  document.addEventListener("kiki:world-send", worldSend);
  window.addEventListener("aethyr-npc", npcLine);
  document.addEventListener("kiki:world-people", worldPeople);
  window.addEventListener("resize", resize);

  return () => {
    observer.disconnect();
    unsubscribe();
    window.clearInterval(timer);
    window.clearTimeout(noticeTimer);
    cancelAnimationFrame(frame);
    document.removeEventListener("click", click, true);
    document.removeEventListener("pointerdown", down);
    document.removeEventListener("pointermove", moveTo);
    document.removeEventListener("pointerup", up);
    document.removeEventListener("pointercancel", up);
    document.removeEventListener("keydown", key, true);
    document.removeEventListener("kiki:world-message", worldMessage);
    document.removeEventListener("kiki:world-send", worldSend);
    window.removeEventListener("aethyr-npc", npcLine);
    document.removeEventListener("kiki:world-people", worldPeople);
    window.removeEventListener("resize", resize);
    document.documentElement.removeAttribute("data-kiki-editing");
    for (const node of document.querySelectorAll("[data-kiki-bar]"))
      if (!element.contains(node)) node.remove();
    root.unmount();
    registry.dispose();
    element.remove();
  };
}
