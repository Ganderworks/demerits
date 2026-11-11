//! Helpers for the demerits test levels: paragraph builders, a scorer, two
//! oracles, a greedy breaker and a seeded random generator.
//!
//! Everything here is written from the contract in demerits' crate docs, not
//! from an implementation. The scorer (`evaluate`) judges any set of breaks
//! line by line. `brute_force` tries every set of breaks on small paragraphs.
//! `oracle` is an exhaustive dynamic program over (break, fitness, line count)
//! for bigger ones, and the unit tests at the bottom check it against
//! `brute_force`.

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub use demerits::{Break, Error, INFINITY, Item, Params, Stretch, break_lines};

// ---------------------------------------------------------------------------
// Building paragraphs

/// The width of one character in [`text`], and the unit the random
/// paragraphs are measured in.
pub const CH: i64 = 1000;

pub fn bx(width: i64) -> Item {
    Item::Box { width }
}

pub fn glue(width: i64, stretch: i64, shrink: i64) -> Item {
    Item::Glue { width, stretch: Stretch::Finite(stretch), shrink }
}

/// Glue that stretches without limit.
pub fn infinite_glue(width: i64, shrink: i64) -> Item {
    Item::Glue { width, stretch: Stretch::Infinite, shrink }
}

pub fn penalty(width: i64, cost: i64, flagged: bool) -> Item {
    Item::Penalty { width, cost, flagged }
}

/// A break that must happen.
pub fn forced() -> Item {
    penalty(0, -INFINITY, false)
}

/// A place that must not break.
pub fn forbidden() -> Item {
    penalty(0, INFINITY, false)
}

/// Glue that stretches without limit (TeX's `\hfil`).
pub fn fill() -> Item {
    infinite_glue(0, 0)
}

/// End a paragraph the way TeX does: no break before the fill, the fill
/// takes up the last line's slack, then the final forced break.
pub fn finish(items: &mut Vec<Item>) {
    items.extend([forbidden(), fill(), forced()]);
}

/// The interword space `text` uses: one character wide, stretching by half
/// that and shrinking by a quarter.
pub const SPACE: Item = Item::Glue { width: CH, stretch: Stretch::Finite(CH / 2), shrink: CH / 4 };

/// The hyphenation point `text` uses: a hyphen one character wide, cost 50,
/// flagged.
pub const HYPHEN: Item = Item::Penalty { width: CH, cost: 50, flagged: true };

/// Monospace text: every character is [`CH`] wide, words are separated by
/// [`SPACE`], a `-` inside a word is a hyphenation point ([`HYPHEN`]), and a
/// lone `/` ends a line (fill, then a forced break). The paragraph is
/// finished with [`finish`] unless it already ends in a forced break.
pub fn text(s: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let mut after_word = false;
    for token in s.split_whitespace() {
        if token == "/" {
            items.extend([forbidden(), fill(), forced()]);
            after_word = false;
            continue;
        }
        if after_word {
            items.push(SPACE);
        }
        for (i, part) in token.split('-').enumerate() {
            if i > 0 {
                items.push(HYPHEN);
            }
            items.push(bx(part.chars().count() as i64 * CH));
        }
        after_word = true;
    }
    if !items.last().is_some_and(is_forced) {
        finish(&mut items);
    }
    items
}

/// Put a hyphenation point every three letters in words of seven or more,
/// keeping at least two letters after the last one. Not English
/// hyphenation, just plenty of flagged breaks.
pub fn hyphenate(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let chars: Vec<char> = word.chars().collect();
            if chars.len() < 7 {
                return word.to_string();
            }
            let mut out = String::new();
            for (i, c) in chars.iter().enumerate() {
                if i > 0 && i % 3 == 0 && chars.len() - i >= 2 {
                    out.push('-');
                }
                out.push(*c);
            }
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Augustine, *Confessions* I.1, Pusey's translation (1838).
pub const CONFESSIONS: &str = "Great art Thou, O Lord, and greatly to be praised; \
great is Thy power, and Thy wisdom infinite. And Thee would man praise; man, but a \
particle of Thy creation; man, that bears about him his mortality, the witness of his \
sin, the witness that Thou resistest the proud: yet would man praise Thee; he, but a \
particle of Thy creation. Thou awakest us to delight in Thy praise; for Thou madest us \
for Thyself, and our heart is restless, until it repose in Thee.";

/// Psalm 23 (KJV), one verse per line.
pub const PSALM_23: &str = "The LORD is my shepherd; I shall not want. / \
He maketh me to lie down in green pastures: he leadeth me beside the still waters. / \
He restoreth my soul: he leadeth me in the paths of righteousness for his name's sake. / \
Yea, though I walk through the valley of the shadow of death, I will fear no evil: \
for thou art with me; thy rod and thy staff they comfort me. / \
Thou preparest a table before me in the presence of mine enemies: thou anointest my \
head with oil; my cup runneth over. / \
Surely goodness and mercy shall follow me all the days of my life: and I will dwell \
in the house of the LORD for ever. /";

// ---------------------------------------------------------------------------
// The contract, one rule at a time

pub fn is_forced(item: &Item) -> bool {
    matches!(*item, Item::Penalty { cost, .. } if cost <= -INFINITY)
}

pub fn is_flagged(item: &Item) -> bool {
    matches!(*item, Item::Penalty { flagged: true, .. })
}

pub fn is_legal_break(items: &[Item], i: usize) -> bool {
    match items[i] {
        Item::Penalty { cost, .. } => cost < INFINITY,
        Item::Glue { .. } => i > 0 && matches!(items[i - 1], Item::Box { .. }),
        Item::Box { .. } => false,
    }
}

/// Where the line after a break at `after` starts (`None`: the paragraph's
/// start).
pub fn line_start(items: &[Item], after: Option<usize>) -> usize {
    let Some(a) = after else { return 0 };
    let mut j = a + 1;
    while j < items.len() && !matches!(items[j], Item::Box { .. }) && !is_forced(&items[j]) {
        j += 1;
    }
    j
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measure {
    pub natural: i64,
    /// The finite stretch only.
    pub stretch: i64,
    /// Whether any glue in the line stretches without limit.
    pub infinite: bool,
    pub shrink: i64,
}

/// Measure the line `start..end` that breaks at `end`.
pub fn measure(items: &[Item], start: usize, end: usize) -> Measure {
    let mut m = Measure { natural: 0, stretch: 0, infinite: false, shrink: 0 };
    for item in &items[start..end] {
        match *item {
            Item::Box { width } => m.natural += width,
            Item::Glue { width, stretch, shrink } => {
                m.natural += width;
                match stretch {
                    Stretch::Finite(y) => m.stretch += y,
                    Stretch::Infinite => m.infinite = true,
                }
                m.shrink += shrink;
            }
            Item::Penalty { .. } => {}
        }
    }
    if let Item::Penalty { width, .. } = items[end] {
        m.natural += width;
    }
    m
}

pub fn ratio(m: Measure, width: i64) -> f64 {
    if m.natural < width {
        if m.infinite {
            0.0
        } else if m.stretch > 0 {
            (width - m.natural) as f64 / m.stretch as f64
        } else {
            f64::INFINITY
        }
    } else if m.natural > width {
        if m.shrink > 0 { (width - m.natural) as f64 / m.shrink as f64 } else { f64::NEG_INFINITY }
    } else {
        0.0
    }
}

pub fn feasible(r: f64, tolerance: f64) -> bool {
    r.is_finite() && r >= -1.0 && r <= tolerance
}

/// 0 tight, 1 decent, 2 loose, 3 very loose.
pub fn fitness(r: f64) -> u8 {
    if r < -0.5 {
        0
    } else if r <= 0.5 {
        1
    } else if r <= 1.0 {
        2
    } else {
        3
    }
}

pub fn badness(r: f64) -> f64 {
    let a = r.abs();
    100.0 * a * a * a
}

pub fn line_width(widths: &[i64], k: usize) -> i64 {
    widths[k.min(widths.len() - 1)]
}

pub fn line_demerits(
    params: &Params,
    r: f64,
    end: &Item,
    previous: Option<&Item>,
    previous_fitness: u8,
    this_fitness: u8,
) -> f64 {
    let base = params.line_penalty + badness(r);
    let base = base * base;
    let p = match *end {
        Item::Penalty { cost, .. } => cost as f64,
        _ => 0.0,
    };
    let mut d = if p >= 0.0 {
        base + p * p
    } else if p > -INFINITY as f64 {
        base - p * p
    } else {
        base
    };
    if is_flagged(end) && previous.is_some_and(is_flagged) {
        d += params.flagged_demerits;
    }
    if this_fitness.abs_diff(previous_fitness) > 1 {
        d += params.fitness_demerits;
    }
    d
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub demerits: f64,
    pub ratios: Vec<f64>,
    pub fitness: Vec<u8>,
}

/// Score a set of breaks, or say exactly which rule it breaks.
pub fn evaluate(
    items: &[Item],
    widths: &[i64],
    params: &Params,
    breaks: &[usize],
) -> Result<Evaluation, String> {
    let mut ev = Evaluation { demerits: 0.0, ratios: Vec::new(), fitness: Vec::new() };
    if items.is_empty() {
        return if breaks.is_empty() { Ok(ev) } else { Err("an empty paragraph has no lines".into()) };
    }
    let n = items.len();
    if breaks.last() != Some(&(n - 1)) {
        return Err(format!("the last break must be the final item ({}), got {:?}", n - 1, breaks.last()));
    }
    if breaks.windows(2).any(|w| w[0] >= w[1]) {
        return Err(format!("break positions must strictly increase: {breaks:?}"));
    }
    for &b in breaks {
        if b >= n || !is_legal_break(items, b) {
            return Err(format!("{b} is not a legal break ({:?})", items.get(b)));
        }
    }
    for (i, item) in items.iter().enumerate() {
        if is_forced(item) && !breaks.contains(&i) {
            return Err(format!("the forced break at {i} was skipped"));
        }
    }
    let mut previous: Option<usize> = None;
    let mut previous_fitness = 1;
    for (k, &b) in breaks.iter().enumerate() {
        let s = line_start(items, previous);
        if b < s {
            return Err(format!(
                "line {k} would break at {b}, inside the run discarded after the break at {previous:?} (it starts at {s})"
            ));
        }
        let w = line_width(widths, k);
        let m = measure(items, s, b);
        let r = ratio(m, w);
        if !feasible(r, params.tolerance) {
            return Err(format!(
                "line {k} ({s}..={b}) isn't feasible: ratio {r} (natural {}, stretch {}{}, shrink {}, width {w}, tolerance {})",
                m.natural,
                m.stretch,
                if m.infinite { " + infinite" } else { "" },
                m.shrink,
                params.tolerance
            ));
        }
        let f = fitness(r);
        ev.demerits += line_demerits(params, r, &items[b], previous.map(|a| &items[a]), previous_fitness, f);
        ev.ratios.push(r);
        ev.fitness.push(f);
        previous = Some(b);
        previous_fitness = f;
    }
    Ok(ev)
}

/// Check what `break_lines` returned: legal, feasible, and with the ratios
/// the contract gives.
pub fn validate(
    items: &[Item],
    widths: &[i64],
    params: &Params,
    breaks: &[Break],
) -> Result<Evaluation, String> {
    let positions: Vec<usize> = breaks.iter().map(|b| b.position).collect();
    let ev = evaluate(items, widths, params, &positions)?;
    for (k, (got, want)) in breaks.iter().map(|b| b.ratio).zip(&ev.ratios).enumerate() {
        // Exactly: the contract says how to compute it.
        if got != *want {
            return Err(format!("line {k} reports ratio {got:?}, but its ratio is {want:?}"));
        }
    }
    Ok(ev)
}

/// Demerits are sums of many lines, and the order you add them in is yours,
/// so they're compared to a relative 1e-9.
pub fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// The spec's guarantee of optimality: between consecutive legal breaks, a
/// line's natural width minus its shrink never goes down.
pub fn right_monotone(items: &[Item]) -> bool {
    let breaks: Vec<usize> = (0..items.len()).filter(|&i| is_legal_break(items, i)).collect();
    let pen = |i: usize| match items[i] {
        Item::Penalty { width, .. } => width,
        _ => 0,
    };
    breaks.windows(2).all(|w| {
        let between: i64 = items[w[0]..w[1]]
            .iter()
            .map(|it| match *it {
                Item::Box { width } => width,
                Item::Glue { width, shrink, .. } => width - shrink,
                Item::Penalty { .. } => 0,
            })
            .sum();
        between - pen(w[0]) + pen(w[1]) >= 0
    })
}

// ---------------------------------------------------------------------------
// The oracles

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expected {
    Infeasible,
    Solution {
        lines: usize,
        demerits: f64,
        /// Several line counts tie for the optimum, so the line count (and,
        /// with looseness, the demerits) can't be checked.
        ambiguous: bool,
    },
}

/// Pick the answer from the fewest demerits at each line count.
pub fn select(counts: &BTreeMap<usize, f64>, looseness: i32) -> Expected {
    if counts.is_empty() {
        return Expected::Infeasible;
    }
    let best = counts.values().copied().fold(f64::INFINITY, f64::min);
    let optimal: Vec<usize> = counts.iter().filter(|(_, d)| close(**d, best)).map(|(k, _)| *k).collect();
    let n = optimal[0] as i64;
    let l = looseness as i64;
    let mut chosen = n;
    for &k in counts.keys() {
        let d = k as i64 - n;
        let within = if l >= 0 { (0..=l).contains(&d) } else { (l..=0).contains(&d) };
        if within && d.abs() > (chosen - n).abs() {
            chosen = k as i64;
        }
    }
    let lines = chosen as usize;
    Expected::Solution { lines, demerits: counts[&lines], ambiguous: optimal.len() > 1 }
}

/// Try every set of breaks. Small paragraphs only.
pub fn brute_force(items: &[Item], widths: &[i64], params: &Params) -> Expected {
    let n = items.len();
    let mandatory: Vec<usize> = (0..n).filter(|&i| is_forced(&items[i])).collect();
    let optional: Vec<usize> = (0..n).filter(|&i| is_legal_break(items, i) && !is_forced(&items[i])).collect();
    assert!(optional.len() <= 20, "brute force on {} optional breaks would take forever", optional.len());
    let mut counts = BTreeMap::new();
    for mask in 0u32..(1 << optional.len()) {
        let mut breaks = mandatory.clone();
        breaks.extend(optional.iter().enumerate().filter(|(bit, _)| mask >> bit & 1 == 1).map(|(_, &b)| b));
        breaks.sort_unstable();
        if let Ok(ev) = evaluate(items, widths, params, &breaks) {
            let e = counts.entry(breaks.len()).or_insert(f64::INFINITY);
            if ev.demerits < *e {
                *e = ev.demerits;
            }
        }
    }
    select(&counts, params.looseness)
}

/// An exhaustive dynamic program: the fewest demerits for every reachable
/// (break, fitness class, line count). Exact for any input.
pub fn oracle(items: &[Item], widths: &[i64], params: &Params) -> Expected {
    assert!(items.last().is_some_and(is_forced), "the oracle needs a terminated paragraph");
    let n = items.len();
    let breaks: Vec<usize> = (0..n).filter(|&i| is_legal_break(items, i)).collect();
    let mut next_forced = vec![n; n + 1];
    for i in (0..n).rev() {
        next_forced[i] = if is_forced(&items[i]) { i } else { next_forced[i + 1] };
    }
    // Moving a line's start earlier only adds width when nothing has a
    // negative (width - shrink); then an overfull line stays overfull.
    let prunable = items.iter().all(|it| match *it {
        Item::Box { width } => width >= 0,
        Item::Glue { width, shrink, .. } => width - shrink >= 0,
        Item::Penalty { .. } => true,
    });
    let widest = widths.iter().copied().max().unwrap_or(i64::MIN);

    // states[0] is the paragraph's start; states[j + 1] is breaks[j].
    let mut states: Vec<BTreeMap<(u8, usize), f64>> = vec![BTreeMap::new(); breaks.len() + 1];
    states[0].insert((1, 0), 0.0);
    for j in 0..breaks.len() {
        let b = breaks[j];
        let mut here: BTreeMap<(u8, usize), f64> = BTreeMap::new();
        for i in (0..=j).rev() {
            let a = if i == 0 { None } else { Some(breaks[i - 1]) };
            let lo = a.map_or(0, |a| a + 1);
            if next_forced[lo] < b {
                break;
            }
            let s = line_start(items, a);
            if b < s {
                continue;
            }
            let m = measure(items, s, b);
            if prunable && m.natural - m.shrink > widest {
                break;
            }
            for (&(pf, k), &total) in &states[i] {
                let r = ratio(m, line_width(widths, k));
                if !feasible(r, params.tolerance) {
                    continue;
                }
                let f = fitness(r);
                let t = total + line_demerits(params, r, &items[b], a.map(|a| &items[a]), pf, f);
                let e = here.entry((f, k + 1)).or_insert(f64::INFINITY);
                if t < *e {
                    *e = t;
                }
            }
        }
        states[j + 1] = here;
    }
    let mut counts: BTreeMap<usize, f64> = BTreeMap::new();
    for (&(_, k), &t) in &states[breaks.len()] {
        let e = counts.entry(k).or_insert(f64::INFINITY);
        if t < *e {
            *e = t;
        }
    }
    select(&counts, params.looseness)
}

/// First fit: each line takes as much as it can without being overfull.
/// What most text engines do. Its lines may well be infeasible.
pub fn greedy(items: &[Item], widths: &[i64]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut previous = None;
    loop {
        let s = line_start(items, previous);
        let w = line_width(widths, out.len());
        let mut chosen = None;
        for b in s..items.len() {
            if !is_legal_break(items, b) {
                continue;
            }
            let m = measure(items, s, b);
            if m.natural - m.shrink > w && chosen.is_some() {
                break;
            }
            chosen = Some(b);
            if is_forced(&items[b]) {
                break;
            }
        }
        let b = chosen.expect("a terminated paragraph always has a next break");
        out.push(b);
        if b == items.len() - 1 {
            return out;
        }
        previous = Some(b);
    }
}

// ---------------------------------------------------------------------------
// Checking break_lines against an oracle

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Solved,
    Infeasible,
    /// Valid, but the optimum ties across line counts, so only validity was
    /// checked.
    Ambiguous,
}

/// Run `break_lines` and hold it to the dynamic-programming oracle.
pub fn check(items: &[Item], widths: &[i64], params: &Params) -> Verdict {
    judge(items, widths, params, oracle(items, widths, params))
}

/// Run `break_lines` and hold it to brute force.
pub fn check_brute(items: &[Item], widths: &[i64], params: &Params) -> Verdict {
    judge(items, widths, params, brute_force(items, widths, params))
}

fn judge(items: &[Item], widths: &[i64], params: &Params, want: Expected) -> Verdict {
    let got = break_lines(items, widths, params);
    let ctx = || repro(items, widths, params);
    match (got, want) {
        (Ok(breaks), Expected::Solution { lines, demerits, ambiguous }) => {
            let ev = validate(items, widths, params, &breaks).unwrap_or_else(|e| panic!("{e}\n{}", ctx()));
            let at: Vec<usize> = breaks.iter().map(|b| b.position).collect();
            if ambiguous {
                // With looseness 0 the demerits are still known exactly.
                assert!(
                    params.looseness != 0 || close(ev.demerits, demerits),
                    "not optimal: breaks at {at:?} cost {} demerits; the best costs {demerits}\n{}",
                    ev.demerits,
                    ctx()
                );
                return Verdict::Ambiguous;
            }
            assert_eq!(
                breaks.len(),
                lines,
                "wrong number of lines (looseness {}): got breaks at {at:?}\n{}",
                params.looseness,
                ctx()
            );
            assert!(
                close(ev.demerits, demerits),
                "not optimal: breaks at {at:?} cost {} demerits; the best costs {demerits}\n{}",
                ev.demerits,
                ctx()
            );
            Verdict::Solved
        }
        (Err(Error::Infeasible), Expected::Infeasible) => Verdict::Infeasible,
        (got, Expected::Infeasible) => panic!("no feasible solution exists, so expected Err(Infeasible), got {got:?}\n{}", ctx()),
        (Err(e), Expected::Solution { demerits, .. }) => {
            panic!("a solution exists ({demerits} demerits), but got Err({e:?})\n{}", ctx())
        }
    }
}

/// Rust you can paste into a test to reproduce a failure.
pub fn repro(items: &[Item], widths: &[i64], params: &Params) -> String {
    fn num(x: f64) -> String {
        if x == f64::INFINITY { "f64::INFINITY".into() } else { format!("{x:?}") }
    }
    fn int(x: i64) -> String {
        match x {
            i64::MIN => "i64::MIN".into(),
            i64::MAX => "i64::MAX".into(),
            INFINITY => "INFINITY".into(),
            _ if x == -INFINITY => "-INFINITY".into(),
            _ => x.to_string(),
        }
    }
    let mut s = String::from("to reproduce:\n    let items = vec![");
    if items.len() > 400 {
        s += &format!("/* {} items, too many to print */", items.len());
    } else {
        for it in items {
            s += &match *it {
                Item::Box { width } => format!("bx({}), ", int(width)),
                Item::Glue { width, stretch: Stretch::Finite(y), shrink } => {
                    format!("glue({}, {}, {}), ", int(width), int(y), int(shrink))
                }
                Item::Glue { width, stretch: Stretch::Infinite, shrink } => {
                    format!("infinite_glue({}, {}), ", int(width), int(shrink))
                }
                Item::Penalty { width, cost, flagged } => format!("penalty({}, {}, {flagged}), ", int(width), int(cost)),
            };
        }
    }
    s += "];\n";
    let ws: Vec<String> = widths.iter().map(|w| int(*w)).collect();
    s += &format!("    let widths = [{}];\n", ws.join(", "));
    s += &format!(
        "    let params = Params {{ tolerance: {}, line_penalty: {}, flagged_demerits: {}, fitness_demerits: {}, looseness: {} }};\n",
        num(params.tolerance),
        num(params.line_penalty),
        num(params.flagged_demerits),
        num(params.fitness_demerits),
        params.looseness
    );
    s
}

/// Run `f` on its own thread with `stack` bytes of stack, and fail if it
/// takes longer than `limit`. (A thread that overflows its stack takes the
/// whole test binary down with it; Rust can't catch that.)
pub fn within<T: Send + 'static>(
    limit: Duration,
    stack: usize,
    what: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (tx, rx) = mpsc::channel();
    let start = Instant::now();
    let handle = thread::Builder::new()
        .stack_size(stack)
        .spawn(move || {
            let _ = tx.send(f());
        })
        .expect("spawn a thread");
    match rx.recv_timeout(limit) {
        Ok(out) => {
            let _ = handle.join();
            out
        }
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("{what}: gave up after {limit:?}"),
        Err(mpsc::RecvTimeoutError::Disconnected) => match handle.join() {
            Err(panic) => std::panic::resume_unwind(panic),
            Ok(()) => panic!("{what}: finished without an answer after {:.2?}", start.elapsed()),
        },
    }
}

// ---------------------------------------------------------------------------
// Random paragraphs

/// SplitMix64: small, seeded, the same on every platform.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// Uniform in lo..=hi.
    pub fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next_u64() % (hi - lo + 1) as u64) as i64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    pub fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.int(0, xs.len() as i64 - 1) as usize]
    }
}

/// What kind of random paragraph to make.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    pub words: usize,
    /// Chance that a word is split into hyphenated fragments.
    pub hyphens: f64,
    /// Chance of an ordinary penalty (random cost) before a space.
    pub penalties: f64,
    /// Chance of a forced break after a word.
    pub forced: f64,
    /// Whether those forced breaks come with fill, like the end of a verse
    /// line.
    pub fills: bool,
    /// Chance of a negative kern inside a fragment.
    pub kerns: f64,
    /// Widths in half characters and stretch and shrink of a quarter, a half,
    /// one or two characters, so ratios land exactly on the class and
    /// tolerance boundaries, and ties are common.
    pub dyadic: bool,
}

impl Default for Shape {
    fn default() -> Self {
        Shape { words: 30, hyphens: 0.3, penalties: 0.1, forced: 0.0, fills: false, kerns: 0.0, dyadic: false }
    }
}

pub fn paragraph(rng: &mut Rng, shape: Shape) -> Vec<Item> {
    let mut items = Vec::new();
    let mut space_next = false;
    for w in 0..shape.words {
        if space_next {
            if rng.chance(shape.penalties) {
                items.push(penalty(0, rng.int(-200, 300), false));
            }
            items.push(if shape.dyadic {
                glue(CH, rng.pick(&[CH / 2, CH, 2 * CH]), rng.pick(&[CH / 4, CH / 2, CH]))
            } else {
                glue(rng.int(800, 1200), rng.int(200, 1000), rng.int(100, 500))
            });
        }
        let fragments = if rng.chance(shape.hyphens) { rng.int(2, 3) } else { 1 };
        for f in 0..fragments {
            if f > 0 {
                items.push(HYPHEN);
            }
            let width = if shape.dyadic { rng.int(3, 12) * CH / 2 } else { rng.int(1500, 6000) };
            if rng.chance(shape.kerns) {
                // A kern inside the fragment; the fragment's net width stays
                // at least 1.2 characters, wider than a hyphen.
                let kern = rng.int(50, 300);
                let left = rng.int(500, width - 500);
                items.extend([bx(left), bx(-kern), bx(width - left)]);
            } else {
                items.push(bx(width));
            }
        }
        space_next = true;
        if w + 1 < shape.words && rng.chance(shape.forced) {
            if shape.fills {
                items.extend([forbidden(), fill()]);
            }
            items.push(forced());
            space_next = false;
        }
    }
    finish(&mut items);
    items
}

/// Random parameters with looseness 0.
pub fn random_params(rng: &mut Rng) -> Params {
    Params {
        tolerance: rng.pick(&[1.0, 1.5, 2.0, 3.0, 5.0, 10.0]),
        line_penalty: rng.int(0, 20) as f64,
        flagged_demerits: rng.pick(&[0.0, 100.0, 3000.0, 10_000.0]),
        fitness_demerits: rng.pick(&[0.0, 100.0, 3000.0, 10_000.0]),
        looseness: 0,
    }
}

/// One line width between `lo` and `hi` characters, or (with `parshape`) one
/// to four.
pub fn random_widths(rng: &mut Rng, parshape: bool, dyadic: bool, lo: i64, hi: i64) -> Vec<i64> {
    let count = if parshape { rng.int(1, 4) as usize } else { 1 };
    (0..count)
        .map(|_| if dyadic { rng.int(lo * 2, hi * 2) * CH / 2 } else { rng.int(lo * CH, hi * CH) })
        .collect()
}

// ---------------------------------------------------------------------------
// The oracle has to be right before it can judge anyone.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_oracle_agrees_with_brute_force() {
        let mut rng = Rng::new(0x0dd5_eed5);
        let mut solved = 0;
        for case in 0..1500 {
            let dyadic = case % 3 == 0;
            let shape = Shape {
                words: rng.int(2, 9) as usize,
                hyphens: 0.3,
                penalties: 0.2,
                forced: if case % 5 == 0 { 0.15 } else { 0.0 },
                fills: case % 2 == 0,
                kerns: if case % 7 == 0 { 0.3 } else { 0.0 },
                dyadic,
            };
            let items = paragraph(&mut rng, shape);
            if (0..items.len()).filter(|&i| is_legal_break(&items, i) && !is_forced(&items[i])).count() > 16 {
                continue;
            }
            let widths = random_widths(&mut rng, case % 4 == 0, dyadic, 6, 20);
            let mut params = random_params(&mut rng);
            params.looseness = if case % 6 == 0 { rng.int(-2, 2) as i32 } else { 0 };
            let a = oracle(&items, &widths, &params);
            let b = brute_force(&items, &widths, &params);
            match (a, b) {
                (Expected::Infeasible, Expected::Infeasible) => {}
                (
                    Expected::Solution { lines: l1, demerits: d1, ambiguous: x1 },
                    Expected::Solution { lines: l2, demerits: d2, ambiguous: x2 },
                ) => {
                    assert_eq!(x1, x2, "{}", repro(&items, &widths, &params));
                    if !x1 {
                        assert_eq!(l1, l2, "{}", repro(&items, &widths, &params));
                        assert!(close(d1, d2), "{d1} vs {d2}\n{}", repro(&items, &widths, &params));
                        solved += 1;
                    }
                }
                _ => panic!("oracle {a:?}, brute force {b:?}\n{}", repro(&items, &widths, &params)),
            }
        }
        assert!(solved > 500, "only {solved} cases had a clear answer");
    }

    #[test]
    fn text_builds_what_it_says() {
        let items = text("ab c-de /");
        assert_eq!(
            items,
            vec![bx(2 * CH), SPACE, bx(CH), HYPHEN, bx(2 * CH), forbidden(), fill(), forced()]
        );
        assert_eq!(hyphenate("restless creation"), "res-tle-ss cre-ati-on");
    }

    #[test]
    fn generated_paragraphs_keep_the_optimality_promise() {
        let mut rng = Rng::new(7);
        for _ in 0..200 {
            let shape = Shape { words: 40, kerns: 0.3, forced: 0.05, fills: true, ..Shape::default() };
            assert!(right_monotone(&paragraph(&mut rng, shape)));
        }
    }
}
