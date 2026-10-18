"use strict";

// Public JavaScript API of pi-poker-eval.
//
// Card strings are turned into card indices (0–51) here and written straight
// into a fixed input buffer in WebAssembly memory; results are read from a
// fixed output buffer and turned back into objects here. A call therefore
// passes only a few small numbers across, with no allocation or copying in the
// generated glue — much cheaper than passing strings and building objects
// across the boundary.

const wasm = require("./pi_poker_eval.js");

const VARIANTS = { NLH: 0, PLO: 1, PLO5: 2, PLO6: 3 };

const CATEGORIES = [
    undefined,
    "High Card",
    "Pair",
    "Two Pair",
    "Three of a Kind",
    "Straight",
    "Flush",
    "Full House",
    "Four of a Kind",
    "Straight Flush",
];

// Index = (rank - 2) * 4 + suit, with suits in the order c, d, h, s (matches Card::index in Rust).
const RANK_SYMBOLS = ["2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A"];
const SUIT_SYMBOLS = ["c", "d", "h", "s"];
const CARD_NAMES = RANK_SYMBOLS.flatMap((rank) => SUIT_SYMBOLS.map((suit) => rank + suit));
const CARD_INDEX = new Map();
CARD_NAMES.forEach((name, index) => {
    CARD_INDEX.set(name.toLowerCase(), index);
    if (name[0] === "T") CARD_INDEX.set("10" + name[1], index);
});

// The exchange buffers. Views are recreated if WebAssembly memory grows
// (growing replaces the underlying ArrayBuffer).
const [INPUT_ADDRESS, INPUT_LEN, OUTPUT_ADDRESS, OUTPUT_LEN] = wasm._exchange();
const memory = wasm._memory();
let buffer = null;
let input = null;
let output = null;

function views() {
    if (buffer !== memory.buffer) {
        buffer = memory.buffer;
        input = new Uint8Array(buffer, INPUT_ADDRESS, INPUT_LEN);
        output = new Uint32Array(buffer, OUTPUT_ADDRESS, OUTPUT_LEN);
    }
}

// Descriptions depend only on the strength value, and there are only a few
// thousand distinct strengths, so each is fetched from WebAssembly once.
const descriptions = new Map();

function variantCode(variant) {
    const code = typeof variant === "string" ? VARIANTS[variant.toUpperCase()] : undefined;
    if (code === undefined) {
        throw new Error(
            `unknown variant: ${JSON.stringify(String(variant))} (expected NLH, PLO, PLO5 or PLO6)`,
        );
    }
    return code;
}

function cardIndex(card) {
    const index = typeof card === "string" ? CARD_INDEX.get(card.trim().toLowerCase()) : undefined;
    if (index === undefined) throw new Error(`invalid card: ${JSON.stringify(String(card))}`);
    return index;
}

/** Writes `cards` as indices into the input buffer from `at`; returns the next position. */
function writeCards(cards, at, what) {
    if (!Array.isArray(cards)) throw new Error(`${what} must be an array of card strings`);
    if (at + cards.length > INPUT_LEN) throw new Error("too many cards");
    for (let i = 0; i < cards.length; i++) input[at + i] = cardIndex(cards[i]);
    return at + cards.length;
}

/** Throws the library's message if a call returned 0 (failure). */
function check(slots) {
    if (slots === 0) throw new Error(wasm._lastError());
    return slots;
}

function description(strength) {
    let text = descriptions.get(strength);
    if (text === undefined) {
        text = wasm._describe(strength);
        descriptions.set(strength, text);
    }
    return text;
}

/** Reads one hand (strength, n, n card indices) from the output buffer at `at`. Returns [hand, next index]. */
function readHand(at) {
    const strength = output[at];
    const count = output[at + 1];
    const cards = new Array(count);
    for (let i = 0; i < count; i++) cards[i] = CARD_NAMES[output[at + 2 + i]];
    const category = CATEGORIES[strength >>> 20];
    return [
        { strength, category, name: category, description: description(strength), cards },
        at + 2 + count,
    ];
}

function evaluate(variant, hole, board) {
    const code = variantCode(variant);
    views();
    const holeEnd = writeCards(hole, 0, "hole");
    writeCards(board, holeEnd, "board");
    check(wasm._evaluate(code, hole.length, board.length));
    views(); // memory may have grown during the call
    return readHand(0)[0];
}

function showdown(variant, board, holes) {
    const code = variantCode(variant);
    if (!Array.isArray(holes) || !holes.every(Array.isArray)) {
        throw new Error("holes must be an array of arrays of card strings");
    }
    views();
    let at = writeCards(board, 0, "board");
    if (at + holes.length > INPUT_LEN) throw new Error("too many players");
    for (const hole of holes) input[at++] = hole.length;
    for (const hole of holes) at = writeCards(hole, at, "hole");

    check(wasm._showdown(code, board.length, holes.length));
    views(); // memory may have grown during the call
    const winnerCount = output[0];
    const winners = Array.from(output.subarray(1, 1 + winnerCount));
    const hands = new Array(holes.length);
    let next = 1 + winnerCount;
    for (let i = 0; i < holes.length; i++) [hands[i], next] = readHand(next);
    return { hands, winners };
}

function equity(request) {
    return wasm._equity(request);
}

function runoutCount(request) {
    return wasm._runoutCount(request);
}

module.exports = { evaluate, showdown, equity, runoutCount };
