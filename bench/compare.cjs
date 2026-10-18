// Compares the JavaScript build of pi-poker-eval with pokersolver and poker-evaluator,
// called from Node on identical pre-generated hands.
//
// Run (after scripts/build-js.sh):
//   cd bench && npm install && node compare.cjs

const pi = require("../pkg");
const { Hand } = require("pokersolver");
const PE = require("poker-evaluator");

const RANKS = "23456789TJQKA";
const SUITS = "cdhs";
const DECK = [...RANKS].flatMap((r) => [...SUITS].map((s) => r + s));

let seed = 42;
const rnd = (n) => (seed = (seed * 1103515245 + 12345) & 0x7fffffff) % n;
const deal = (n) => {
    const d = DECK.slice();
    for (let i = 0; i < n; i++) {
        const j = i + rnd(52 - i);
        [d[i], d[j]] = [d[j], d[i]];
    }
    return d.slice(0, n);
};

function measure(label, items, iterations, fn) {
    for (let i = 0; i < Math.min(2000, iterations); i++) fn(items[i % items.length]); // warm-up / JIT
    const t0 = process.hrtime.bigint();
    for (let i = 0; i < iterations; i++) fn(items[i % items.length]);
    const ns = Number(process.hrtime.bigint() - t0) / iterations;
    console.log(
        `  ${label.padEnd(44)} ${(ns / 1000).toFixed(3).padStart(10)} µs   ${Math.round(1e9 / ns)
            .toLocaleString()
            .padStart(11)} /s`,
    );
    return ns;
}

const combos = (a, k) =>
    k === 0 ? [[]] : a.flatMap((x, i) => combos(a.slice(i + 1), k - 1).map((r) => [x, ...r]));

/** pokersolver Omaha: rank every 2+3 combination, then pick the best with Hand.winners. */
const solverOmaha = (hole, board) => {
    const all = [];
    for (const h of combos(hole, 2))
        for (const b of combos(board, 3)) all.push(Hand.solve([...h, ...b]));
    return Hand.winners(all)[0];
};
/** poker-evaluator Omaha: best 5-card value over every 2+3 combo (no description available). */
const peOmaha = (hole, board) => {
    let best = 0;
    for (const h of combos(hole, 2))
        for (const b of combos(board, 3)) best = Math.max(best, PE.evalHand([...h, ...b]).value);
    return best;
};

console.log(`node ${process.version} ${process.arch}\n`);
const results = [];
const row = (name, solver, evaluator, piNs) => results.push({ name, solver, evaluator, pi: piNs });

console.log("Hold'em, one 7-card hand");
const nlh = Array.from({ length: 5000 }, () => deal(7));
const a1 = measure("pokersolver  Hand.solve", nlh, 50_000, (d) => Hand.solve(d));
const b1 = measure("poker-evaluator  evalHand (no description)", nlh, 500_000, (d) =>
    PE.evalHand(d),
);
const c1 = measure("pi-poker-eval  evaluate", nlh, 500_000, (d) =>
    pi.evaluate("NLH", d.slice(0, 2), d.slice(2)),
);
row("Hold'em, one hand", a1, b1, c1);

for (const [variant, h] of [
    ["PLO", 4],
    ["PLO6", 6],
]) {
    console.log(`\n${variant}, one hand (${h} hole + 5 board)`);
    const deals = Array.from({ length: 2000 }, () => deal(h + 5));
    const a = measure("pokersolver  (engine method)", deals, 2_000, (d) =>
        solverOmaha(d.slice(0, h), d.slice(h)),
    );
    const b = measure("poker-evaluator  (no description)", deals, 10_000, (d) =>
        peOmaha(d.slice(0, h), d.slice(h)),
    );
    const c = measure("pi-poker-eval  evaluate", deals, 100_000, (d) =>
        pi.evaluate(variant, d.slice(0, h), d.slice(h)),
    );
    row(`${variant}, one hand`, a, b, c);
}

for (const [variant, h] of [
    ["NLH", 2],
    ["PLO", 4],
]) {
    console.log(`\n${variant} showdown, 6 players`);
    const deals = Array.from({ length: 1000 }, () => deal(h * 6 + 5));
    const split = (d) => [
        d.slice(h * 6),
        Array.from({ length: 6 }, (_, p) => d.slice(p * h, (p + 1) * h)),
    ];
    const a = measure(
        "pokersolver  (engine method)",
        deals,
        variant === "NLH" ? 10_000 : 300,
        (d) => {
            const [board, holes] = split(d);
            Hand.winners(
                holes.map((hole) =>
                    variant === "NLH" ? Hand.solve([...hole, ...board]) : solverOmaha(hole, board),
                ),
            );
        },
    );
    const b = measure(
        "poker-evaluator  (no description)",
        deals,
        variant === "NLH" ? 100_000 : 2_000,
        (d) => {
            const [board, holes] = split(d);
            Math.max(
                ...holes.map((hole) =>
                    variant === "NLH"
                        ? PE.evalHand([...hole, ...board]).value
                        : peOmaha(hole, board),
                ),
            );
        },
    );
    const c = measure(
        "pi-poker-eval  showdown",
        deals,
        variant === "NLH" ? 100_000 : 20_000,
        (d) => {
            const [board, holes] = split(d);
            pi.showdown(variant, board, holes);
        },
    );
    row(`${variant} showdown, 6 players`, a, b, c);
}

console.log("\nSummary — time per call, and how many times faster pi-poker-eval is");
console.log(
    "  " +
        "".padEnd(28) +
        "pokersolver".padStart(14) +
        "poker-evaluator".padStart(18) +
        "pi-poker-eval".padStart(16) +
        "   vs solver   vs evaluator",
);
const fmt = (ns) => (ns >= 1000 ? (ns / 1000).toFixed(2) + " µs" : ns.toFixed(0) + " ns");
for (const r of results) {
    console.log(
        "  " +
            r.name.padEnd(28) +
            fmt(r.solver).padStart(14) +
            fmt(r.evaluator).padStart(18) +
            fmt(r.pi).padStart(16) +
            `${(r.solver / r.pi).toFixed(1)}×`.padStart(12) +
            `${(r.evaluator / r.pi).toFixed(1)}×`.padStart(14),
    );
}
console.log(
    `\n  memory: RSS ${(process.memoryUsage().rss / 1e6).toFixed(0)} MB (poker-evaluator loads a 130 MB table; pi-poker-eval's WebAssembly is ~130 KB)`,
);
