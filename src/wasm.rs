//! JavaScript / TypeScript bindings (feature `wasm`).
//!
//! A thin layer over the Rust API, mirroring it one-to-one: cards are strings
//! (`"Ah"`, `"Td"`, `"10d"`), variants are the short codes (`"NLH"`, `"PLO"`,
//! `"PLO5"`, `"PLO6"`), results are plain objects and errors are thrown as
//! `Error` with the library's message.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::{
    Card, Equity, EquityCalculator, EquityMethod, EquityRequest, EvalError, EvaluatedHand,
    Evaluator, HandEvaluator, Variant,
};

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &str = r#"
/** A game variant: Texas Hold'em or Omaha with 4, 5 or 6 hole cards. */
export type Variant = "NLH" | "PLO" | "PLO5" | "PLO6";

/** The type of a poker hand, weakest first. */
export type Category =
    | "High Card" | "Pair" | "Two Pair" | "Three of a Kind" | "Straight"
    | "Flush" | "Full House" | "Four of a Kind" | "Straight Flush";

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

/** Every player's best hand at showdown, and who wins. */
export interface Showdown {
    /** Same order as the hole cards passed in. */
    hands: EvaluatedHand[];
    /** Indices of the winners, lowest first. More than one means a split pot. */
    winners: number[];
}
"#;

#[derive(Serialize)]
struct JsHand {
    strength: u32,
    category: &'static str,
    name: &'static str,
    description: String,
    cards: Vec<String>,
}

impl From<&EvaluatedHand> for JsHand {
    fn from(hand: &EvaluatedHand) -> JsHand {
        JsHand {
            strength: hand.strength.value(),
            category: hand.category.name(),
            name: hand.name(),
            description: hand.description(),
            cards: hand.cards.iter().map(Card::to_string).collect(),
        }
    }
}

#[derive(Serialize)]
struct JsShowdown {
    hands: Vec<JsHand>,
    winners: Vec<usize>,
}

/// The best hand `hole` can make with `board`.
///
/// @param variant - "NLH", "PLO", "PLO5" or "PLO6"
/// @param hole - the player's hole cards, e.g. ["Ah", "Kd"]
/// @param board - 0, 3, 4 or 5 board cards
/// @throws Error on an invalid card, variant, duplicate card or wrong card count
#[wasm_bindgen(unchecked_return_type = "EvaluatedHand")]
pub fn evaluate(
    #[wasm_bindgen(unchecked_param_type = "Variant")] variant: &str,
    hole: Vec<String>,
    board: Vec<String>,
) -> Result<JsValue, JsError> {
    let variant = parse_variant(variant)?;
    let hand = Evaluator
        .evaluate(variant, &parse_cards(&hole)?, &parse_cards(&board)?)
        .map_err(js_error)?;
    to_js(&JsHand::from(&hand))
}

/// Every player's best hand with the shared `board`, plus the winners.
///
/// @param variant - "NLH", "PLO", "PLO5" or "PLO6"
/// @param board - 0, 3, 4 or 5 board cards
/// @param holes - each player's hole cards, e.g. [["Ah","Kd"], ["Qc","Qs"]]
/// @throws Error on an invalid card, variant, duplicate card or wrong card count
#[wasm_bindgen(unchecked_return_type = "Showdown")]
pub fn showdown(
    #[wasm_bindgen(unchecked_param_type = "Variant")] variant: &str,
    board: Vec<String>,
    #[wasm_bindgen(unchecked_param_type = "string[][]")] holes: JsValue,
) -> Result<JsValue, JsError> {
    let variant = parse_variant(variant)?;
    let holes: Vec<Vec<String>> = serde_wasm_bindgen::from_value(holes)
        .map_err(|_| JsError::new("holes must be an array of arrays of card strings"))?;
    let holes = holes
        .iter()
        .map(|h| parse_cards(h))
        .collect::<Result<Vec<_>, _>>()?;
    let refs: Vec<&[Card]> = holes.iter().map(Vec::as_slice).collect();

    let result = Evaluator
        .showdown(variant, &parse_cards(&board)?, &refs)
        .map_err(js_error)?;
    to_js(&JsShowdown {
        hands: result.hands.iter().map(JsHand::from).collect(),
        winners: result.winners,
    })
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum JsMethod {
    Exact,
    MonteCarlo {
        trials: u32,
        seed: u64,
    },
    #[serde(rename_all = "camelCase")]
    Auto {
        max_exact: u64,
        trials: u32,
        seed: u64,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JsEquityRequest {
    variant: String,
    players: Vec<Vec<String>>,
    board: Vec<String>,
    #[serde(default)]
    dead: Vec<String>,
    method: JsMethod,
}

#[derive(Serialize)]
struct JsPlayerEquity {
    win: f64,
    tie: f64,
    equity: f64,
}

#[derive(Serialize)]
struct JsEquityResult {
    players: Vec<JsPlayerEquity>,
    runouts: u64,
    exact: bool,
}

fn to_request(value: JsValue) -> Result<EquityRequest, JsError> {
    let req: JsEquityRequest = serde_wasm_bindgen::from_value(value)
        .map_err(|e| JsError::new(&format!("invalid equity request: {e}")))?;
    Ok(EquityRequest {
        variant: parse_variant(&req.variant)?,
        players: req
            .players
            .iter()
            .map(|p| parse_cards(p))
            .collect::<Result<_, _>>()?,
        board: parse_cards(&req.board)?,
        dead: parse_cards(&req.dead)?,
        method: match req.method {
            JsMethod::Exact => EquityMethod::Exact,
            JsMethod::MonteCarlo { trials, seed } => EquityMethod::MonteCarlo { trials, seed },
            JsMethod::Auto {
                max_exact,
                trials,
                seed,
            } => EquityMethod::Auto {
                max_exact,
                trials,
                seed,
            },
        },
    })
}

/// How often each player wins: exact over every run-out, or seeded Monte Carlo.
///
/// @param request - variant, players (an empty array is a random opponent), board,
///   optional dead cards, and method
/// @throws Error on an invalid request, card, duplicate, or too many unknown cards
#[wasm_bindgen(unchecked_return_type = "EquityResult")]
pub fn equity(
    #[wasm_bindgen(unchecked_param_type = "EquityRequest")] request: JsValue,
) -> Result<JsValue, JsError> {
    let result = Equity.equity(&to_request(request)?).map_err(js_error)?;
    to_js(&JsEquityResult {
        players: result
            .players
            .iter()
            .map(|p| JsPlayerEquity {
                win: p.win,
                tie: p.tie,
                equity: p.equity,
            })
            .collect(),
        runouts: result.runouts,
        exact: result.exact,
    })
}

/// How many run-outs an exact calculation would evaluate (beyond 2^53 the number is approximate).
///
/// @param request - the same shape as for `equity`; `method` is ignored
#[wasm_bindgen(js_name = runoutCount)]
pub fn runout_count(
    #[wasm_bindgen(unchecked_param_type = "EquityRequest")] request: JsValue,
) -> Result<f64, JsError> {
    Ok(Equity
        .runout_count(&to_request(request)?)
        .map_err(js_error)? as f64)
}

fn parse_variant(text: &str) -> Result<Variant, JsError> {
    text.parse()
        .map_err(|message: String| JsError::new(&message))
}

fn parse_cards(texts: &[String]) -> Result<Vec<Card>, JsError> {
    texts
        .iter()
        .map(|t| t.parse::<Card>().map_err(js_error))
        .collect()
}

fn js_error(error: EvalError) -> JsError {
    JsError::new(&error.to_string())
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsError::new(&e.to_string()))
}
