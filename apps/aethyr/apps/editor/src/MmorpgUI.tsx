import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type FormEvent,
} from "react";
import type { EditorService } from "../../../packages/editor-services/service.ts";
import type { EngineStatus } from "../../../packages/editor-services/protocol.ts";
import type { Receipt } from "../../../packages/editor-services/state.ts";
import type {
  MmoIconKey,
  MmoItem,
  MmoPlayerClass,
  MmoUiAction,
} from "../../../packages/runtime/mmorpg-protocol.ts";
import {
  MMO_ITEM_NAMES,
  MMO_STAT_ROWS,
  mmoLogLines,
  mmoUiImageURL,
} from "./mmorpg-ui-model.ts";
import "./mmorpg-ui.css";

type Panel = "stats" | "inventory" | "quests" | null;
interface Props {
  service: EditorService;
  status: EngineStatus;
  receipt: Receipt | null;
  error: string | null;
  editing?: boolean;
}
const EMPTY_INPUT = {
  forward: 0,
  right: 0,
  up: 0,
  lookX: 0,
  lookY: 0,
  sprint: false,
  jump: false,
  attack: false,
  dance: false,
};
const ITEM_ICON: Record<MmoItem, MmoIconKey> = {
  "weapon.sword1": "sword",
  "weapon.axe1": "axe",
  "weapon.hammer1": "hammer",
};
const SLOT_MIME = "application/x-aethyrforge-inventory-slot";
const background = (url?: string): CSSProperties =>
  url ? { backgroundImage: `url("${url}")` } : {};

/** Source-style game panels. React observes snapshots, never advances the world. */
export function MmorpgUI({
  service,
  status,
  receipt,
  error,
  editing = false,
}: Props) {
  const state = status.mmorpg!;
  const [name, setName] = useState("");
  const [playerClass, setPlayerClass] = useState<MmoPlayerClass>("paladin");
  const [panel, setPanel] = useState<Panel>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [chat, setChat] = useState("");
  const [notice, setNotice] = useState("");
  const [loginFade, setLoginFade] = useState(false);
  const previousOnline = useRef(false);
  const [pending, setPending] = useState<{
    id: string;
    type: MmoUiAction["type"];
    text?: string;
  } | null>(null);
  const chatInput = useRef<HTMLInputElement>(null);
  const log = useRef<HTMLDivElement>(null);
  const player = state.player;
  const online = state.connection === "online" && !!player;
  useEffect(() => {
    if (!editing) return;
    setPanel(null);
    setSelected(null);
    chatInput.current?.blur();
  }, [editing]);
  useLayoutEffect(() => {
    const entered = online && !previousOnline.current;
    previousOnline.current = online;
    if (!entered) {
      if (!online) setLoginFade(false);
      return;
    }
    document.getElementById("preview-frame")?.focus();
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    setLoginFade(true);
    const timer = setTimeout(() => setLoginFade(false), 1000);
    return () => clearTimeout(timer);
  }, [online]);
  const icon = (key: MmoIconKey): string | undefined => {
    const descriptor = state.icons?.icons[key];
    return descriptor
      ? mmoUiImageURL(descriptor.url, location.origin)
      : undefined;
  };
  const resetInput = () => service.send({ type: "input", input: EMPTY_INPUT });
  const send = (action: MmoUiAction) => {
    if (pending) return;
    setNotice("");
    try {
      setPending({
        id: service.mmorpg(action),
        type: action.type,
        text: action.type === "chat" ? action.text : undefined,
      });
    } catch (e) {
      setNotice(
        e instanceof Error ? e.message : "Couldn't send that. Try again.",
      );
    }
  };
  useEffect(() => {
    if (!pending || receipt?.requestId !== pending.id) return;
    if (!receipt.ok)
      setNotice(receipt.message || "Couldn't complete that. Try again.");
    else if (pending.type === "chat")
      setChat((value) => (value.trim() === pending.text ? "" : value));
    setPending(null);
  }, [receipt, pending]);
  useEffect(() => {
    if (!pending) return;
    const timer = setTimeout(() => {
      setPending(null);
      setNotice("The request took too long. Try again.");
    }, 17_000);
    return () => clearTimeout(timer);
  }, [pending]);
  useEffect(() => {
    if (online) setPending((p) => (p?.type === "join" ? null : p));
  }, [online]);
  useEffect(
    () =>
      service.onMmorpgFocusChat(() => {
        if (online && !editing) chatInput.current?.focus();
      }),
    [online, editing, service],
  );
  const returnToWorld = () => {
    setPanel(null);
    setSelected(null);
    resetInput();
    chatInput.current?.blur();
    document.getElementById("preview-frame")?.focus();
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (editing) return;
      if (e.key === "Escape" && online) {
        e.preventDefault();
        returnToWorld();
      }
      if (
        e.key === "Enter" &&
        online &&
        !(e.target instanceof HTMLInputElement) &&
        !(e.target instanceof HTMLSelectElement) &&
        !(e.target instanceof HTMLTextAreaElement)
      ) {
        e.preventDefault();
        chatInput.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [online, editing]);
  const lines = player ? mmoLogLines(state.events, player, state.actors) : [];
  const lastLine = lines.at(-1)?.id;
  useEffect(() => {
    if (log.current) log.current.scrollTop = log.current.scrollHeight;
  }, [lastLine]);
  const swap = (from: number, to: number) => {
    setSelected(null);
    if (!player || from === to || player.inventoryVersion === undefined) return;
    send({ type: "swap", version: player.inventoryVersion, from, to });
  };
  const slot = (index: number) => {
    const item = player?.inventory?.[index] ?? null;
    const label = `${index >= 24 ? `Equipment ${index - 23}` : `Backpack ${index + 1}`}: ${item ? MMO_ITEM_NAMES[item] : "Empty"}`;
    return (
      <button
        key={index}
        type="button"
        className={`mmo-slot${selected === index ? " mmo-slot--selected" : ""}`}
        data-slot={index}
        aria-label={label}
        title={`${label}${index === 24 ? " · Weapon hand" : ""}`}
        aria-pressed={selected === index}
        disabled={!!pending || !player?.alive}
        draggable={!!item && !pending}
        onClick={() =>
          selected === null ? setSelected(index) : swap(selected, index)
        }
        onDragStart={(e) => {
          e.dataTransfer.setData(SLOT_MIME, String(index));
          e.dataTransfer.effectAllowed = "move";
          setSelected(index);
        }}
        onDragEnd={() => setSelected(null)}
        onDragOver={(e) => {
          if (e.dataTransfer.types.includes(SLOT_MIME)) {
            e.preventDefault();
            e.dataTransfer.dropEffect = "move";
          }
        }}
        onDrop={(e) => {
          e.preventDefault();
          const value = e.dataTransfer.getData(SLOT_MIME);
          if (/^(?:[0-9]|[12][0-9]|3[01])$/.test(value))
            swap(Number(value), index);
        }}
      >
        {item && icon(ITEM_ICON[item]) ? (
          <img src={icon(ITEM_ICON[item])} alt="" draggable={false} />
        ) : null}
      </button>
    );
  };
  const login = (e: FormEvent) => {
    e.preventDefault();
    if (name.trim())
      send({ type: "join", name: name.trim(), class: playerClass });
  };
  const blocked = pending?.type === "join" || state.connection === "connecting";
  const showLogin = !online;
  return (
    <div
      className={`mmo-ui${editing ? " mmo-ui--editing" : ""}`}
      data-connection={state.connection}
      onFocusCapture={resetInput}
      onKeyDownCapture={(e) => {
        if (e.key === "Escape" && online) {
          e.preventDefault();
          e.stopPropagation();
          returnToWorld();
        }
      }}
    >
      {showLogin || loginFade ? (
        <div
          className={`mmo-login${loginFade ? " mmo-login--leaving" : ""}`}
          style={background(icon("loginBackground"))}
          aria-hidden={loginFade || undefined}
          inert={loginFade || undefined}
        >
          <form
            className="mmo-login-window"
            onSubmit={login}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <h1>Enter the world</h1>
            <label htmlFor="mmo-name">Guest name</label>
            <input
              id="mmo-name"
              autoComplete="nickname"
              maxLength={64}
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={blocked}
              required
            />
            <label htmlFor="mmo-class">Character</label>
            <select
              id="mmo-class"
              value={playerClass}
              onChange={(e) => setPlayerClass(e.target.value as MmoPlayerClass)}
              disabled={blocked}
            >
              <option value="paladin">Paladin</option>
              <option value="sorceror">Sorceror</option>
            </select>
            <button
              className="mmo-login-button"
              type="submit"
              disabled={blocked || !name.trim()}
            >
              {blocked
                ? "Entering…"
                : state.connection === "disconnected"
                  ? "Reconnect"
                  : "Enter world"}
            </button>
            <p role="status" aria-live="polite">
              {notice ||
                state.message ||
                (status.phase === "error"
                  ? status.message
                  : "Choose a name for this visit.")}
            </p>
          </form>
        </div>
      ) : null}
      {online ? (
        <>
          <section
            className="mmo-health"
            aria-label="Player health"
            style={background(icon("healthFrame"))}
          >
            <div
              className="mmo-health-track"
              role="progressbar"
              aria-label="Health"
              aria-valuemin={0}
              aria-valuemax={player.maxHealth}
              aria-valuenow={player.health}
            >
              <span
                style={{
                  width: `${(100 * player.health) / player.maxHealth}%`,
                }}
              />
            </div>
            <span className="mmo-health-number">
              {Math.ceil(player.health)} / {Math.ceil(player.maxHealth)}
            </span>
          </section>
          <output className="mmo-fps" aria-label="Frames per second">
            {Math.round(status.fps)} FPS
          </output>
          <nav className="mmo-icon-bar" aria-label="Character panels">
            {(
              [
                ["stats", "Statistics"],
                ["inventory", "Inventory"],
                ["quests", "Quest journal"],
              ] as const
            ).map(([key, title]) => (
              <button
                key={key}
                type="button"
                className="mmo-icon"
                style={background(icon(key))}
                aria-label={title}
                title={title}
                aria-pressed={panel === key}
                onClick={() => {
                  setPanel(panel === key ? null : key);
                  setSelected(null);
                }}
              />
            ))}
          </nav>
          {panel ? (
            <section
              className={`mmo-panel mmo-panel--${panel}`}
              aria-label={
                panel === "stats"
                  ? "Statistics"
                  : panel === "inventory"
                    ? "Inventory"
                    : "Quest journal"
              }
            >
              <button
                type="button"
                className="mmo-panel-close"
                aria-label="Close panel"
                onClick={() => setPanel(null)}
              >
                ×
              </button>
              <h2>
                {panel === "stats"
                  ? "Statistics"
                  : panel === "inventory"
                    ? "Inventory"
                    : "Quest journal"}
              </h2>
              {panel === "stats" ? (
                <dl className="mmo-stats">
                  {MMO_STAT_ROWS.map(({ key, label, tooltip }) => (
                    <div key={key}>
                      <dt>
                        <span
                          className="mmo-stats-tooltip"
                          data-stat={key}
                          tabIndex={0}
                          aria-describedby={`mmo-stat-tooltip-${key}`}
                        >
                          <span className="mmo-stat-label">{label}</span>
                          <span
                            className="mmo-stats-tooltip-text"
                            id={`mmo-stat-tooltip-${key}`}
                            role="tooltip"
                          >
                            {tooltip}
                          </span>
                        </span>
                      </dt>
                      <dd>{player.stats[key]}</dd>
                    </div>
                  ))}
                </dl>
              ) : panel === "inventory" ? (
                <>
                  <div className="mmo-equipment">
                    <div className="mmo-equipment-column">
                      {[24, 25, 26, 27].map(slot)}
                    </div>
                    <div
                      className="mmo-inventory-portrait"
                      style={background(icon("inventoryPortrait"))}
                      role="img"
                      aria-label="Character equipment"
                    />
                    <div className="mmo-equipment-column">
                      {[28, 29, 30, 31].map(slot)}
                    </div>
                  </div>
                  <div className="mmo-backpack">
                    {Array.from({ length: 24 }, (_, i) => slot(i))}
                  </div>
                  <p className="mmo-panel-hint">
                    Drag an item, or select two slots to swap.
                  </p>
                </>
              ) : (
                <p className="mmo-empty-journal">No quests.</p>
              )}
            </section>
          ) : null}
          <section className="mmo-chat" aria-label="Chat and combat log">
            <div
              className="mmo-chat-log"
              ref={log}
              role="log"
              aria-live="polite"
              aria-relevant="additions text"
            >
              {lines.map((line) => (
                <p
                  className={`mmo-chat-line mmo-chat-line--${line.kind}`}
                  key={line.id}
                >
                  {line.text}
                </p>
              ))}
            </div>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                if (chat.trim()) send({ type: "chat", text: chat.trim() });
              }}
              onKeyDown={(e) => e.stopPropagation()}
            >
              <input
                ref={chatInput}
                className="mmo-chat-input"
                aria-label="Chat message"
                placeholder="Press Enter to chat"
                maxLength={64}
                value={chat}
                onChange={(e) => setChat(e.target.value)}
                disabled={pending?.type === "chat"}
                autoComplete="off"
              />
              <button
                type="submit"
                className="mmo-chat-send"
                disabled={!!pending || !chat.trim()}
                aria-label="Send chat message"
              >
                ↵
              </button>
            </form>
          </section>
          {!player.alive ? (
            <div className="mmo-death" role="status">
              <h2>You have fallen</h2>
              <button
                type="button"
                className="mmo-login-button"
                disabled={!!pending}
                onClick={() => send({ type: "respawn" })}
              >
                {pending?.type === "respawn"
                  ? "Returning…"
                  : "Return to the world"}
              </button>
            </div>
          ) : null}
        </>
      ) : null}
      {!showLogin && (notice || error) ? (
        <p className="mmo-notice" role="alert">
          {notice || error}
        </p>
      ) : null}
    </div>
  );
}
