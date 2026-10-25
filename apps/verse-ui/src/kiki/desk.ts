// The pure half of Kiki's house editor desk: which cards are open, where
// each one sits, the guest one-change rule, and how chat and payout lines
// read. The DOM half is `desk-host.tsx`.
import { CARDS, type Card, type Command } from "./commands";

export interface Point {
  readonly x: number;
  readonly y: number;
  /** The card's width when it was moved, so it keeps its size. */
  readonly w?: number;
}

export interface DeskState {
  /** Cards the visitor closed. */
  readonly closed: ReadonlyArray<Card>;
  /** Cards the visitor moved, by their top-left corner. */
  readonly moved: Readonly<Partial<Record<Card, Point>>>;
}

/** The desk's own cards, which share the top of a phone screen. */
export const DESK_CARDS: ReadonlyArray<Card> = [
  "chat",
  "inventory",
  "payouts",
  "actions",
];

/** On a phone the desk's own cards start closed and open on request. */
export const initialDesk = (phone: boolean): DeskState => ({
  closed: phone ? CARDS.filter((card) => DESK_CARDS.includes(card)) : [],
  moved: {},
});

export const isOpen = (state: DeskState, card: Card) =>
  !state.closed.includes(card);

/**
 * Open or close a card. On a phone (`single`), opening one of the desk's
 * own cards closes the others, so they never cover each other.
 */
export function apply(
  state: DeskState,
  command: Command,
  single = false,
): DeskState {
  if (single && command.verb === "open" && DESK_CARDS.includes(command.card))
    state = {
      ...state,
      closed: [
        ...state.closed.filter((card) => !DESK_CARDS.includes(card)),
        ...DESK_CARDS.filter((card) => card !== command.card),
      ],
    };
  const closed = state.closed.filter((card) => card !== command.card);
  return {
    ...state,
    closed: command.verb === "close" ? [...closed, command.card] : closed,
  };
}

/** How much of a card's header must stay on screen to grab it again. */
export const GRIP = 48;

/** Keep a moved card's header reachable inside the window. */
export function clamp(
  point: Point,
  size: { readonly width: number },
  view: { readonly width: number; readonly height: number },
): Point {
  const x = Math.min(
    Math.max(point.x, GRIP - size.width),
    Math.max(0, view.width - GRIP),
  );
  const y = Math.min(Math.max(point.y, 0), Math.max(0, view.height - GRIP));
  return { x: Math.round(x), y: Math.round(y) };
}

export const move = (state: DeskState, card: Card, to: Point): DeskState => ({
  ...state,
  moved: { ...state.moved, [card]: to },
});

/** Read a saved desk back, dropping anything that isn't one. */
export function restore(text: string | null, fallback: DeskState): DeskState {
  if (!text) return fallback;
  try {
    const value = JSON.parse(text) as Partial<DeskState>;
    const closed = Array.isArray(value.closed)
      ? CARDS.filter((card) => value.closed!.includes(card))
      : fallback.closed;
    const moved: Partial<Record<Card, Point>> = {};
    for (const card of CARDS) {
      const point = (value.moved as Record<string, Point> | undefined)?.[card];
      if (point && Number.isFinite(point.x) && Number.isFinite(point.y))
        moved[card] = {
          x: point.x,
          y: point.y,
          ...(Number.isFinite(point.w) && point.w! > 0 && { w: point.w }),
        };
    }
    return { closed, moved };
  } catch {
    return fallback;
  }
}

// --- Guest practice -------------------------------------------------------

/** What the house shows when someone tries to make a change. */
export interface Attempt {
  /** The visitor is a guest (practising, not building their own world). */
  readonly guest: boolean;
  /** Changes already made on this visit. */
  readonly changes: number;
  /** The confirmation is for saving the world, not for a change. */
  readonly saving: boolean;
}

/** A guest keeps one change at a time: a second one asks for a revert. */
export const blocksChange = (attempt: Attempt) =>
  attempt.guest && !attempt.saving && attempt.changes >= 1;

/** The house control that takes the guest's change back, if it offers one. */
export function revertControl(controls: {
  readonly undo: { readonly shown: boolean; readonly enabled: boolean };
  readonly revert: { readonly shown: boolean; readonly enabled: boolean };
}): "edit-undo" | "project-revert" | undefined {
  if (controls.undo.shown && controls.undo.enabled) return "edit-undo";
  if (controls.revert.shown && controls.revert.enabled) return "project-revert";
  return undefined;
}

// --- Chat -----------------------------------------------------------------

export type Room = "world" | "agent";

export interface Message {
  readonly id: string;
  readonly name: string;
  readonly text: string;
  /** Milliseconds since the epoch. */
  readonly at: number;
  /** Written by this visitor (drawn on the right). */
  readonly mine: boolean;
}

export const MAX_MESSAGES = 200;

export const append = (
  log: ReadonlyArray<Message>,
  message: Message,
): ReadonlyArray<Message> => [...log, message].slice(-MAX_MESSAGES);

/** "14:05", in the visitor's own clock. */
export const clock = (at: number) =>
  new Date(at).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });

// --- Edits and payouts ------------------------------------------------------

/**
 * A line from the house's change history as the payouts card shows it. The
 * house says "Not recorded" when no payment was recorded for a change, which
 * is a payment of 0; every other line is shown as the house wrote it.
 */
export function payoutLine(text: string): string {
  const match = /^(earnings:\s*)?not recorded\.?$/i.exec(text.trim());
  return match ? "Paid: 0" : text;
}
