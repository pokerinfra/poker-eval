// Tests for the JavaScript build (pkg/), run with `node --test js-test/`.
// They call the package exactly as a JavaScript or TypeScript user would.

const test = require("node:test");
const assert = require("node:assert/strict");
const { evaluate, showdown } = require("../pkg/pi_poker_eval.js");

test("evaluate returns a plain object with every field", () => {
    const hand = evaluate("PLO", ["Tc", "9c", "Qc", "Ac"], ["3c", "5h", "6c", "9s", "7c"]);
    assert.deepEqual(hand, {
        strength: hand.strength,
        category: "Flush",
        name: "Flush",
        description: "Flush, A High",
        cards: ["Ac", "Qc", "7c", "6c", "3c"],
    });
    assert.equal(typeof hand.strength, "number");
});

test("card strings: T or 10, any letter case", () => {
    const a = evaluate("NLH", ["10h", "td"], ["Ac", "7s", "2d"]);
    const b = evaluate("NLH", ["Th", "Td"], ["ac", "7S", "2D"]);
    assert.equal(a.strength, b.strength);
    assert.equal(a.description, "Pair, 10's");
});

test("partial boards and preflop", () => {
    assert.equal(evaluate("NLH", ["Ah", "Ad"], []).description, "Pair, A's");
    assert.equal(
        evaluate("PLO6", ["Ah", "Kh", "Qc", "Js", "Tc", "9d"], ["2h", "5h", "8h"]).description,
        "Flush, A High",
    );
});

test("showdown: an A-high flush beats a J-high flush in Omaha", () => {
    const result = showdown(
        "PLO",
        ["3c", "5h", "6c", "9s", "7c"],
        [
            ["Tc", "9c", "Qc", "Ac"],
            ["Jc", "2d", "2c", "Ts"],
        ],
    );
    assert.deepEqual(result.winners, [0]);
    assert.equal(result.hands[1].description, "Flush, J High");
});

test("showdown: split pot when the board plays", () => {
    const result = showdown(
        "NLH",
        ["Ah", "Kh", "Qh", "Jh", "Th"],
        [
            ["2c", "3d"],
            ["4c", "5d"],
        ],
    );
    assert.deepEqual(result.winners, [0, 1]);
    assert.equal(result.hands[0].description, "Royal Flush");
});

test("strength compares directly as a number", () => {
    const sixHigh = evaluate("NLH", ["6h", "Qc"], ["2c", "3d", "4s", "5h", "Kc"]);
    const wheel = evaluate("NLH", ["Ah", "Qd"], ["2c", "3d", "4s", "5h", "Kc"]);
    assert.ok(sixHigh.strength > wheel.strength);
    assert.deepEqual(wheel.cards, ["5h", "4s", "3d", "2c", "Ah"]);
});

test("errors are thrown with the library's message", () => {
    assert.throws(() => evaluate("NLH", ["Ah", "1h"], []), { message: 'invalid card: "1h"' });
    assert.throws(() => evaluate("PLO7", ["Ah", "Kh"], []), /unknown variant/);
    assert.throws(() => evaluate("PLO", ["Ah", "Kh", "Qh"], []), {
        message: "PLO needs 4 hole cards, got 3",
    });
    assert.throws(() => evaluate("NLH", ["Ah", "Kh"], ["Ah", "2c", "3c"]), {
        message: "duplicate card: Ah",
    });
    assert.throws(() => evaluate("NLH", ["Ah", "Kh"], ["2c"]), /invalid board of 1 cards/);
    assert.throws(() => showdown("NLH", [], []), { message: "no players given" });
    assert.throws(() => showdown("NLH", [], "nope"), /holes must be an array/);
});

// ---- Equity ----
const { equity, runoutCount } = require("../pkg/pi_poker_eval.js");

test("equity: exact AhAs vs KdKc preflop matches the independent count", () => {
    const r = equity({
        variant: "NLH",
        players: [
            ["Ah", "As"],
            ["Kd", "Kc"],
        ],
        board: [],
        method: { type: "exact" },
    });
    assert.equal(r.runouts, 1712304);
    assert.equal(r.exact, true);
    // Independently enumerated with poker-evaluator: equity 0.8125549, win 0.8106458, tie 0.0038182.
    assert.ok(Math.abs(r.players[0].equity - 0.8125549) < 1e-6);
    assert.ok(Math.abs(r.players[0].win - 0.8106458) < 1e-6);
    assert.ok(Math.abs(r.players[0].tie - 0.0038182) < 1e-6);
});

test("equity: Monte Carlo with random opponents, repeatable by seed", () => {
    const req = {
        variant: "PLO",
        players: [["Ah", "Ad", "Kh", "Qd"], []],
        board: [],
        method: { type: "monteCarlo", trials: 5000, seed: 7 },
    };
    const a = equity(req);
    assert.deepEqual(a, equity(req));
    assert.equal(a.exact, false);
    assert.equal(a.runouts, 5000);
    assert.ok(Math.abs(a.players[0].equity + a.players[1].equity - 1) < 1e-9);
});

test("equity: auto, dead cards and runoutCount", () => {
    const req = {
        variant: "NLH",
        players: [
            ["Ah", "As"],
            ["Kd", "Kc"],
        ],
        board: ["2c", "7d", "9s"],
        dead: ["3h"],
        method: { type: "auto", maxExact: 10000, trials: 1000, seed: 1 },
    };
    // 52 - 4 hole - 3 board - 1 dead = 44 cards left; turn + river = C(44, 2) = 946.
    assert.equal(runoutCount(req), 946);
    const r = equity(req);
    assert.equal(r.exact, true);
    assert.equal(r.runouts, 946);
});

test("equity: invalid requests throw", () => {
    assert.throws(
        () =>
            equity({
                variant: "NLH",
                players: [["Ah", "As"]],
                board: [],
                method: { type: "magic" },
            }),
        /invalid equity request/,
    );
    assert.throws(
        () =>
            equity({
                variant: "NLH",
                players: [["Ah", "As"]],
                board: [],
                method: { type: "monteCarlo", trials: 0, seed: 1 },
            }),
        { message: "Monte Carlo needs at least one trial" },
    );
    assert.throws(
        () => equity({ variant: "NLH", players: [], board: [], method: { type: "exact" } }),
        { message: "no players given" },
    );
});
