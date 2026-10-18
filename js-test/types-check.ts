// Compile-only check that the generated TypeScript types are usable and strict.
// Run: tsc --noEmit --strict js-test/types-check.ts

import { evaluate, showdown, type EvaluatedHand, type Showdown, type Variant } from "../pkg";

const variant: Variant = "PLO";
const hand: EvaluatedHand = evaluate(variant, ["Ah", "Ad", "Kc", "Qs"], []);
const strength: number = hand.strength;
const cards: string[] = hand.cards;

const result: Showdown = showdown(
    "NLH",
    ["Ah", "Kh", "Qh"],
    [
        ["2c", "3d"],
        ["4c", "5d"],
    ],
);
const winners: number[] = result.winners;

// @ts-expect-error — only the four variant codes are accepted.
evaluate("PLO7", ["Ah", "Kh"], []);

export { strength, cards, winners };

import { equity, runoutCount, type EquityResult } from "../pkg";
const eq: EquityResult = equity({
    variant: "NLH",
    players: [["Ah", "As"], []],
    board: [],
    method: { type: "monteCarlo", trials: 1000, seed: 1 },
});
const share: number = eq.players[0].equity;
const count: number = runoutCount({
    variant: "PLO",
    players: [["Ah", "Ad", "Kh", "Qd"], []],
    board: ["2c", "7d", "9s"],
    method: { type: "exact" },
});
// @ts-expect-error — Monte Carlo needs trials and a seed.
equity({ variant: "NLH", players: [["Ah", "As"]], board: [], method: { type: "monteCarlo" } });
export { share, count };
