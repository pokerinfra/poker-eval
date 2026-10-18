/** A game variant: Texas Hold'em or Omaha with 4, 5 or 6 hole cards. */
export type Variant = "NLH" | "PLO" | "PLO5" | "PLO6";

/** The type of a poker hand, weakest first. */
export type Category =
    | "High Card"
    | "Pair"
    | "Two Pair"
    | "Three of a Kind"
    | "Straight"
    | "Flush"
    | "Full House"
    | "Four of a Kind"
    | "Straight Flush";

/** A player's best hand. */
export interface EvaluatedHand {
    /** Comparable strength: higher wins, equal is a tie. */
    strength: number;
    category: Category;
    /** Same as `category`; kept for readability at call sites. */
    name: Category;
    /** e.g. "Flush, A High", "Two Pair, K's & 4's", "Royal Flush". */
    description: string;
    /**
     * The best five cards (fewer on a short hand), in display order,
     * e.g. ["Ac", "Qc", "7c", "6c", "3c"].
     */
    cards: string[];
}

/** Every player's best hand at showdown, and who wins. */
export interface Showdown {
    /** Same order as the hole cards passed in. */
    hands: EvaluatedHand[];
    /** Indices of the winners, lowest first. More than one means a split pot. */
    winners: number[];
}

/** How to calculate equity. */
export type EquityMethod =
    /** Every possible run-out. */
    | { type: "exact" }
    /** `trials` random run-outs; the same seed always gives the same answer. */
    | { type: "monteCarlo"; trials: number; seed: number }
    /** Exact when there are at most `maxExact` run-outs, otherwise Monte Carlo. */
    | { type: "auto"; maxExact: number; trials: number; seed: number };

/** What to calculate equity for. */
export interface EquityRequest {
    variant: Variant;
    /** Each player's hole cards; an empty array is an unknown opponent (random hand). */
    players: string[][];
    /** Known board cards: 0, 3, 4 or 5. */
    board: string[];
    /** Cards known to be out of play. Optional. */
    dead?: string[];
    method: EquityMethod;
}

/** One player's result; each value is a fraction from 0 to 1. */
export interface PlayerEquity {
    /** Won outright. */
    win: number;
    /** Shared the pot. */
    tie: number;
    /** Average share of the pot: wins plus the player's share of splits. */
    equity: number;
}

export interface EquityResult {
    /** Same order as the request's players. */
    players: PlayerEquity[];
    /** How many run-outs were evaluated. */
    runouts: number;
    /** true if every run-out was evaluated, false for Monte Carlo. */
    exact: boolean;
}

/**
 * The best hand `hole` can make with `board`.
 *
 * @param variant - "NLH", "PLO", "PLO5" or "PLO6"
 * @param hole - the player's hole cards, e.g. ["Ah", "Kd"]
 * @param board - 0, 3, 4 or 5 board cards
 * @throws Error on an invalid card, variant, duplicate card or wrong card count
 */
export function evaluate(variant: Variant, hole: string[], board: string[]): EvaluatedHand;

/**
 * Every player's best hand with the shared `board`, plus the winners.
 *
 * @param variant - "NLH", "PLO", "PLO5" or "PLO6"
 * @param board - 0, 3, 4 or 5 board cards
 * @param holes - each player's hole cards, e.g. [["Ah", "Kd"], ["Qc", "Qs"]]
 * @throws Error on an invalid card, variant, duplicate card or wrong card count
 */
export function showdown(variant: Variant, board: string[], holes: string[][]): Showdown;

/**
 * How often each player wins: exact over every run-out, or seeded Monte Carlo.
 *
 * @throws Error on an invalid request, card, duplicate, or too many unknown cards
 */
export function equity(request: EquityRequest): EquityResult;

/** How many run-outs an exact calculation would evaluate (beyond 2^53 the number is approximate). */
export function runoutCount(request: EquityRequest): number;
