//! Low-level WebAssembly exports (feature `wasm`).
//!
//! These are internal: the npm package's public API is `js/index.cjs`, which
//! converts card strings to card indices (0–51) and back, and builds the result
//! objects on the JavaScript side. Passing one small numeric array each way,
//! instead of many strings and object fields, keeps the cost of crossing
//! between JavaScript and WebAssembly low.

use std::cell::{RefCell, UnsafeCell};

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::{
    Card, Equity, EquityCalculator, EquityMethod, EquityRequest, EvalError, EvaluatedHand,
    Evaluator, HandEvaluator, HandStrength, Rank, Suit, Variant,
};

// ---------------------------------------------------------------------------
// Hand evaluation through fixed exchange buffers
// ---------------------------------------------------------------------------

const INPUT_LEN: usize = 512;
const OUTPUT_LEN: usize = 512;

/// Fixed exchange buffers in WebAssembly memory. JavaScript writes card indices
/// into `input` and reads results from `output`, so an evaluation allocates
/// nothing and copies nothing through the generated glue.
struct Exchange {
    input: UnsafeCell<[u8; INPUT_LEN]>,
    output: UnsafeCell<[u32; OUTPUT_LEN]>,
}

// SAFETY: WebAssembly here runs on a single thread, and the buffers are only
// touched inside `exchange`, one call at a time.
unsafe impl Sync for Exchange {}

static EXCHANGE: Exchange = Exchange {
    input: UnsafeCell::new([0; INPUT_LEN]),
    output: UnsafeCell::new([0; OUTPUT_LEN]),
};

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Internal. The WebAssembly memory the exchange buffers live in.
#[wasm_bindgen(js_name = _memory)]
pub fn memory() -> JsValue {
    wasm_bindgen::memory()
}

/// Internal. `[input address, input bytes, output address, output u32 slots]`.
#[wasm_bindgen(js_name = _exchange)]
pub fn exchange_layout() -> Vec<u32> {
    vec![
        EXCHANGE.input.get() as u32,
        INPUT_LEN as u32,
        EXCHANGE.output.get() as u32,
        OUTPUT_LEN as u32,
    ]
}

/// Internal. The message of the last failed call (one that returned 0).
#[wasm_bindgen(js_name = _lastError)]
pub fn last_error() -> String {
    LAST_ERROR.with(|e| e.borrow().clone())
}

/// Internal. Input: `hole_len` hole cards then `board_len` board cards.
/// Output: `[strength, n, card₁ … cardₙ]`. Returns the number of output slots, or 0 on error.
#[wasm_bindgen(js_name = _evaluate)]
pub fn evaluate_exchange(variant: u8, hole_len: usize, board_len: usize) -> usize {
    exchange(|input, output| {
        // Generous room, so oversized requests still reach the evaluator and get
        // its own message (e.g. "NLH needs 2 hole cards, got 20").
        const MAX_CARDS: usize = 64;
        let n = hole_len + board_len;
        if n > MAX_CARDS {
            return Err("too many cards".to_string());
        }
        let mut cards = [Card::new(Rank::Two, Suit::Clubs); MAX_CARDS];
        decode(&input[..n], &mut cards[..n])?;
        let hand = Evaluator
            .evaluate(
                variant_from_code(variant)?,
                &cards[..hole_len],
                &cards[hole_len..n],
            )
            .map_err(|e| e.to_string())?;
        let mut at = 0;
        write_hand(output, &mut at, &hand);
        Ok(at)
    })
}

/// Internal. Input: `board_len` board cards, then `players` hole-card counts,
/// then every player's hole cards back to back. Output:
/// `[w, winner₁ … winner_w, then per player: strength, n, card₁ … cardₙ]`.
/// Returns the number of output slots, or 0 on error.
#[wasm_bindgen(js_name = _showdown)]
pub fn showdown_exchange(variant: u8, board_len: usize, players: usize) -> usize {
    exchange(|input, output| {
        let counts = input
            .get(board_len..board_len + players)
            .ok_or("too many players")?;
        let total: usize = counts.iter().map(|&c| c as usize).sum();
        let start = board_len + players;
        if start + total > INPUT_LEN || 1 + players * 8 > OUTPUT_LEN {
            return Err("too many players or cards".to_string());
        }
        let mut cards = vec![Card::new(Rank::Two, Suit::Clubs); board_len + total];
        decode(&input[..board_len], &mut cards[..board_len])?;
        decode(&input[start..start + total], &mut cards[board_len..])?;
        let (board, all_holes) = cards.split_at(board_len);

        let mut holes: Vec<&[Card]> = Vec::with_capacity(players);
        let mut from = 0;
        for &count in counts {
            holes.push(&all_holes[from..from + count as usize]);
            from += count as usize;
        }
        let result = Evaluator
            .showdown(variant_from_code(variant)?, board, &holes)
            .map_err(|e| e.to_string())?;

        output[0] = result.winners.len() as u32;
        for (i, &w) in result.winners.iter().enumerate() {
            output[1 + i] = w as u32;
        }
        let mut at = 1 + result.winners.len();
        for hand in &result.hands {
            write_hand(output, &mut at, hand);
        }
        Ok(at)
    })
}

/// Internal. The description of a hand strength, e.g. `"Flush, A High"`.
#[wasm_bindgen(js_name = _describe)]
pub fn describe_strength(strength: u32) -> Result<String, JsError> {
    HandStrength::from_value(strength)
        .map(crate::describe::description)
        .ok_or_else(|| JsError::new("invalid hand strength"))
}

/// Runs `call` with the exchange buffers; on error records the message and returns 0.
fn exchange(call: impl FnOnce(&[u8], &mut [u32]) -> Result<usize, String>) -> usize {
    // SAFETY: single-threaded, and no other reference to the buffers is alive
    // while `call` runs (nothing it calls touches EXCHANGE).
    let (input, output) = unsafe { (&*EXCHANGE.input.get(), &mut *EXCHANGE.output.get()) };
    match call(input, output) {
        Ok(slots) => slots,
        Err(message) => {
            LAST_ERROR.with(|e| *e.borrow_mut() = message);
            0
        }
    }
}

fn write_hand(output: &mut [u32], at: &mut usize, hand: &EvaluatedHand) {
    output[*at] = hand.strength.value();
    output[*at + 1] = hand.cards.len() as u32;
    for (i, card) in hand.cards.iter().enumerate() {
        output[*at + 2 + i] = card.index() as u32;
    }
    *at += 2 + hand.cards.len();
}

fn decode(indices: &[u8], cards: &mut [Card]) -> Result<(), String> {
    for (slot, &index) in cards.iter_mut().zip(indices) {
        *slot = Card::from_index(index).ok_or("invalid card index")?;
    }
    Ok(())
}

fn variant_from_code(code: u8) -> Result<Variant, String> {
    match code {
        0 => Ok(Variant::Holdem),
        1 => Ok(Variant::Omaha4),
        2 => Ok(Variant::Omaha5),
        3 => Ok(Variant::Omaha6),
        _ => Err("invalid variant code".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Equity (one call per request; the computation dwarfs the crossing cost)
// ---------------------------------------------------------------------------

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

/// Internal. Equity for a request object (see `EquityRequest` in js/index.d.ts).
#[wasm_bindgen(js_name = _equity)]
pub fn equity(request: JsValue) -> Result<JsValue, JsError> {
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

/// Internal. Run-out count for a request object (beyond 2^53 the number is approximate).
#[wasm_bindgen(js_name = _runoutCount)]
pub fn runout_count(request: JsValue) -> Result<f64, JsError> {
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
