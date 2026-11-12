"""Helpers for the Python trials: paragraph builders, a scorer, two oracles, a
greedy breaker and a seeded random generator.

A line-for-line port of `trials/src/lib.rs`. The random generator is the same
SplitMix64, so the same seed makes the same paragraph in both languages.

Written from the contract, not from an implementation. `evaluate` judges any
set of breaks line by line. `brute_force` tries every way to break a small
paragraph into feasible lines. `oracle` is an exhaustive dynamic program over
(break, fitness, line count), and `test_trials.py` checks it against
`brute_force`.
"""

from __future__ import annotations

import enum
import math
from collections.abc import Sequence
from dataclasses import dataclass

from demerits import (
    INFINITE,
    INFINITY,
    Box,
    Break,
    DemeritsError,
    Glue,
    Infeasible,
    Item,
    Params,
    Penalty,
    Unterminated,
    break_lines,
)

__all__ = [
    # demerits
    "INFINITE", "INFINITY", "Box", "Break", "DemeritsError", "Glue", "Infeasible", "Item", "Params",
    "Penalty", "Unterminated", "break_lines",
    # building paragraphs
    "CH", "bx", "glue", "infinite_glue", "penalty", "forced", "forbidden", "fill", "finish", "SPACE",
    "HYPHEN", "text", "hyphenate", "CONFESSIONS", "PSALM_23",
    # the contract
    "is_forced", "is_flagged", "is_legal_break", "line_start", "Measure", "measure", "ratio",
    "feasible", "fitness", "badness", "line_width", "line_demerits", "Evaluation", "Invalid",
    "evaluate", "validate", "close", "right_monotone",
    # judging
    "Solution", "select", "brute_force", "oracle", "greedy", "Verdict", "check", "check_brute", "repro",
    # random paragraphs
    "Rng", "Shape", "paragraph", "random_params", "random_widths",
]

# ---------------------------------------------------------------------------
# Building paragraphs

#: The width of one character in `text`, and the unit the random paragraphs
#: are measured in.
CH = 1000


def bx(width: int) -> Item:
    return Box(width)


def glue(width: int, stretch: int, shrink: int) -> Item:
    return Glue(width, stretch, shrink)


def infinite_glue(width: int, shrink: int) -> Item:
    """Glue that stretches without limit."""
    return Glue(width, INFINITE, shrink)


def penalty(width: int, cost: int, flagged: bool) -> Item:
    return Penalty(width, cost, flagged)


def forced() -> Item:
    """A break that must happen."""
    return penalty(0, -INFINITY, False)


def forbidden() -> Item:
    """A place that must not break."""
    return penalty(0, INFINITY, False)


def fill() -> Item:
    r"""Glue that stretches without limit (TeX's `\hfil`)."""
    return infinite_glue(0, 0)


def finish(items: list[Item]) -> None:
    """End a paragraph the way TeX does: no break before the fill, the fill
    takes up the last line's slack, then the final forced break."""
    items.extend([forbidden(), fill(), forced()])


#: The interword space `text` uses: one character wide, stretching by half
#: that and shrinking by a quarter.
SPACE = Glue(CH, CH // 2, CH // 4)

#: The hyphenation point `text` uses: a hyphen one character wide, cost 50,
#: flagged.
HYPHEN = Penalty(CH, 50, True)


def text(s: str) -> list[Item]:
    """Monospace text: every character is `CH` wide, words are separated by
    `SPACE`, a `-` inside a word is a hyphenation point (`HYPHEN`), and a lone
    `/` ends a line (fill, then a forced break). The paragraph is finished
    with `finish` unless it already ends in a forced break."""
    items: list[Item] = []
    after_word = False
    for token in s.split():
        if token == "/":
            items.extend([forbidden(), fill(), forced()])
            after_word = False
            continue
        if after_word:
            items.append(SPACE)
        for i, part in enumerate(token.split("-")):
            if i > 0:
                items.append(HYPHEN)
            items.append(bx(len(part) * CH))
        after_word = True
    if not (items and is_forced(items[-1])):
        finish(items)
    return items


def hyphenate(s: str) -> str:
    """Put a hyphenation point every three letters in words of seven or more,
    keeping at least two letters after the last one. Not English
    hyphenation, just plenty of flagged breaks."""
    words = []
    for word in s.split():
        if len(word) < 7:
            words.append(word)
            continue
        out = []
        for i, c in enumerate(word):
            if i > 0 and i % 3 == 0 and len(word) - i >= 2:
                out.append("-")
            out.append(c)
        words.append("".join(out))
    return " ".join(words)


#: Augustine, *Confessions* I.1, Pusey's translation (1838).
CONFESSIONS = (
    "Great art Thou, O Lord, and greatly to be praised; "
    "great is Thy power, and Thy wisdom infinite. And Thee would man praise; man, but a "
    "particle of Thy creation; man, that bears about him his mortality, the witness of his "
    "sin, the witness that Thou resistest the proud: yet would man praise Thee; he, but a "
    "particle of Thy creation. Thou awakest us to delight in Thy praise; for Thou madest us "
    "for Thyself, and our heart is restless, until it repose in Thee."
)

#: Psalm 23 (KJV), one verse per line.
PSALM_23 = (
    "The LORD is my shepherd; I shall not want. / "
    "He maketh me to lie down in green pastures: he leadeth me beside the still waters. / "
    "He restoreth my soul: he leadeth me in the paths of righteousness for his name's sake. / "
    "Yea, though I walk through the valley of the shadow of death, I will fear no evil: "
    "for thou art with me; thy rod and thy staff they comfort me. / "
    "Thou preparest a table before me in the presence of mine enemies: thou anointest my "
    "head with oil; my cup runneth over. / "
    "Surely goodness and mercy shall follow me all the days of my life: and I will dwell "
    "in the house of the LORD for ever. /"
)

# ---------------------------------------------------------------------------
# The contract, one rule at a time


def is_forced(item: Item) -> bool:
    return isinstance(item, Penalty) and item.cost <= -INFINITY


def is_flagged(item: Item) -> bool:
    return isinstance(item, Penalty) and item.flagged


def is_legal_break(items: Sequence[Item], i: int) -> bool:
    match items[i]:
        case Penalty(cost=cost):
            return cost < INFINITY
        case Glue():
            return i > 0 and isinstance(items[i - 1], Box)
        case _:
            return False


def line_start(items: Sequence[Item], after: int | None) -> int:
    """Where the line after a break at `after` starts (`None`: the
    paragraph's start)."""
    if after is None:
        return 0
    j = after + 1
    while j < len(items) and not isinstance(items[j], Box) and not is_forced(items[j]):
        j += 1
    return j


@dataclass(frozen=True)
class Measure:
    natural: int
    #: The finite stretch only.
    stretch: int
    #: Whether any glue in the line stretches without limit.
    infinite: bool
    shrink: int


def measure(items: Sequence[Item], start: int, end: int) -> Measure:
    """Measure the line `start..end` that breaks at `end`."""
    natural = stretch = shrink = 0
    infinite = False
    for item in items[start:end]:
        match item:
            case Box(width=w):
                natural += w
            case Glue(width=w, stretch=y, shrink=z):
                natural += w
                if y is INFINITE:
                    infinite = True
                else:
                    stretch += y
                shrink += z
    end_item = items[end]
    if isinstance(end_item, Penalty):
        natural += end_item.width
    return Measure(natural, stretch, infinite, shrink)


def ratio(m: Measure, width: int) -> float:
    if m.natural < width:
        if m.infinite:
            return 0.0
        if m.stretch > 0:
            return float(width - m.natural) / float(m.stretch)
        return math.inf
    if m.natural > width:
        if m.shrink > 0:
            return float(width - m.natural) / float(m.shrink)
        return -math.inf
    return 0.0


def feasible(r: float, tolerance: float) -> bool:
    return math.isfinite(r) and -1.0 <= r <= tolerance


def fitness(r: float) -> int:
    """0 tight, 1 decent, 2 loose, 3 very loose."""
    if r < -0.5:
        return 0
    if r <= 0.5:
        return 1
    if r <= 1.0:
        return 2
    return 3


def badness(r: float) -> float:
    a = abs(r)
    return 100.0 * a * a * a


def line_width(widths: Sequence[int], k: int) -> int:
    return widths[min(k, len(widths) - 1)]


def line_demerits(
    params: Params,
    r: float,
    end: Item,
    previous: Item | None,
    previous_fitness: int,
    this_fitness: int,
) -> float:
    base = params.line_penalty + badness(r)
    base = base * base
    p = float(end.cost) if isinstance(end, Penalty) else 0.0
    if p >= 0.0:
        d = base + p * p
    elif p > -INFINITY:
        d = base - p * p
    else:
        d = base
    if is_flagged(end) and previous is not None and is_flagged(previous):
        d += params.flagged_demerits
    if abs(this_fitness - previous_fitness) > 1:
        d += params.fitness_demerits
    return d


@dataclass
class Evaluation:
    demerits: float
    ratios: list[float]
    fitness: list[int]


class Invalid(AssertionError):
    """A set of breaks that breaks a rule. The message says which."""


def evaluate(items: Sequence[Item], widths: Sequence[int], params: Params, breaks: Sequence[int]) -> Evaluation:
    """Score a set of breaks, or raise `Invalid` saying exactly which rule it
    breaks."""
    ev = Evaluation(0.0, [], [])
    if not items:
        if breaks:
            raise Invalid("an empty paragraph has no lines")
        return ev
    n = len(items)
    if not breaks or breaks[-1] != n - 1:
        raise Invalid(f"the last break must be the final item ({n - 1}), got {breaks[-1] if breaks else None}")
    if any(a >= b for a, b in zip(breaks, breaks[1:])):
        raise Invalid(f"break positions must strictly increase: {list(breaks)}")
    for b in breaks:
        if not isinstance(b, int) or not 0 <= b < n or not is_legal_break(items, b):
            raise Invalid(f"{b} is not a legal break ({items[b] if isinstance(b, int) and 0 <= b < n else None})")
    chosen = set(breaks)
    for i, item in enumerate(items):
        if is_forced(item) and i not in chosen:
            raise Invalid(f"the forced break at {i} was skipped")
    previous: int | None = None
    previous_fitness = 1
    for k, b in enumerate(breaks):
        s = line_start(items, previous)
        if b < s:
            raise Invalid(
                f"line {k} would break at {b}, inside the run discarded after the break at {previous} "
                f"(it starts at {s})"
            )
        w = line_width(widths, k)
        m = measure(items, s, b)
        r = ratio(m, w)
        if not feasible(r, params.tolerance):
            raise Invalid(
                f"line {k} ({s}..={b}) isn't feasible: ratio {r} (natural {m.natural}, stretch {m.stretch}"
                f"{' + infinite' if m.infinite else ''}, shrink {m.shrink}, width {w}, "
                f"tolerance {params.tolerance})"
            )
        f = fitness(r)
        ev.demerits += line_demerits(
            params, r, items[b], None if previous is None else items[previous], previous_fitness, f
        )
        ev.ratios.append(r)
        ev.fitness.append(f)
        previous = b
        previous_fitness = f
    return ev


def validate(items: Sequence[Item], widths: Sequence[int], params: Params, breaks: Sequence[Break]) -> Evaluation:
    """Check what `break_lines` returned: legal, feasible, and with the ratios
    the contract gives, exactly."""
    if not isinstance(breaks, list) or not all(isinstance(b, Break) for b in breaks):
        raise Invalid(f"break_lines should return a list of Break, got {breaks!r}")
    ev = evaluate(items, widths, params, [b.position for b in breaks])
    for k, (got, want) in enumerate(zip((b.ratio for b in breaks), ev.ratios)):
        if not isinstance(got, float) or got != want:
            raise Invalid(f"line {k} reports ratio {got!r}, but its ratio is {want!r}")
    return ev


def close(a: float, b: float) -> bool:
    """Demerits are sums of many lines, and the order you add them in is
    yours, so they're compared to a relative 1e-9."""
    return abs(a - b) <= 1e-9 * max(abs(a), abs(b), 1.0)


def right_monotone(items: Sequence[Item]) -> bool:
    """The contract's condition for optimality: between consecutive legal
    breaks, a line's natural width minus its shrink never goes down."""
    breaks = [i for i in range(len(items)) if is_legal_break(items, i)]

    def pen(i: int) -> int:
        item = items[i]
        return item.width if isinstance(item, Penalty) else 0

    def net(item: Item) -> int:
        match item:
            case Box(width=w):
                return w
            case Glue(width=w, shrink=z):
                return w - z
            case _:
                return 0

    return all(sum(net(it) for it in items[a:b]) - pen(a) + pen(b) >= 0 for a, b in zip(breaks, breaks[1:]))


# ---------------------------------------------------------------------------
# The oracles


@dataclass(frozen=True)
class Solution:
    """What the best answer looks like. (`None` stands for infeasible.)"""

    lines: int
    demerits: float
    #: Several line counts tie for the optimum, so the line count (and, with
    #: looseness, the demerits) can't be checked.
    ambiguous: bool


def select(counts: dict[int, float], looseness: int) -> Solution | None:
    """Pick the answer from the fewest demerits at each line count."""
    if not counts:
        return None
    best = min(counts.values())
    optimal = [k for k in sorted(counts) if close(counts[k], best)]
    n = optimal[0]
    chosen = n
    for k in sorted(counts):
        d = k - n
        within = 0 <= d <= looseness if looseness >= 0 else looseness <= d <= 0
        if within and abs(d) > abs(chosen - n):
            chosen = k
    return Solution(chosen, counts[chosen], len(optimal) > 1)


def brute_force(items: Sequence[Item], widths: Sequence[int], params: Params) -> Solution | None:
    """Try every way to break the paragraph into feasible lines. Small
    paragraphs only.

    The Rust version tries every subset of breaks and scores each with
    `evaluate`. That's too slow in Python, so this one walks the same subsets
    line by line and stops extending any that already have an infeasible
    line. It still sees every valid answer, and adds up each answer's
    demerits in the same order `evaluate` does.
    """
    assert items and is_forced(items[-1]), "brute force needs a terminated paragraph"
    n = len(items)
    counts: dict[int, float] = {}

    def walk(previous: int | None, previous_fitness: int, k: int, total: float) -> None:
        s = line_start(items, previous)
        w = line_width(widths, k)
        for b in range(s, n):
            if not is_legal_break(items, b):
                continue
            r = ratio(measure(items, s, b), w)
            if feasible(r, params.tolerance):
                f = fitness(r)
                t = total + line_demerits(
                    params, r, items[b], None if previous is None else items[previous], previous_fitness, f
                )
                if b == n - 1:
                    if t < counts.get(k + 1, math.inf):
                        counts[k + 1] = t
                else:
                    walk(b, f, k + 1, t)
            if is_forced(items[b]):
                return  # no line may skip a forced break

    walk(None, 1, 0, 0.0)
    return select(counts, params.looseness)


def oracle(items: Sequence[Item], widths: Sequence[int], params: Params) -> Solution | None:
    """An exhaustive dynamic program: the fewest demerits for every reachable
    (break, fitness class, line count). Exact for any input."""
    assert items and is_forced(items[-1]), "the oracle needs a terminated paragraph"
    n = len(items)
    breaks = [i for i in range(n) if is_legal_break(items, i)]
    next_forced = [n] * (n + 1)
    for i in range(n - 1, -1, -1):
        next_forced[i] = i if is_forced(items[i]) else next_forced[i + 1]

    # Moving a line's start earlier only adds width when nothing has a
    # negative (width - shrink); then an overfull line stays overfull.
    def nonnegative(item: Item) -> bool:
        match item:
            case Box(width=w):
                return w >= 0
            case Glue(width=w, shrink=z):
                return w - z >= 0
            case _:
                return True

    prunable = all(nonnegative(it) for it in items)
    widest = max(widths)

    # states[0] is the paragraph's start; states[j + 1] is breaks[j].
    states: list[dict[tuple[int, int], float]] = [{} for _ in range(len(breaks) + 1)]
    states[0][(1, 0)] = 0.0
    for j, b in enumerate(breaks):
        here: dict[tuple[int, int], float] = {}
        for i in range(j, -1, -1):
            a = None if i == 0 else breaks[i - 1]
            lo = 0 if a is None else a + 1
            if next_forced[lo] < b:
                break
            s = line_start(items, a)
            if b < s:
                continue
            m = measure(items, s, b)
            if prunable and m.natural - m.shrink > widest:
                break
            for (pf, k), total in sorted(states[i].items()):
                r = ratio(m, line_width(widths, k))
                if not feasible(r, params.tolerance):
                    continue
                f = fitness(r)
                t = total + line_demerits(params, r, items[b], None if a is None else items[a], pf, f)
                if t < here.get((f, k + 1), math.inf):
                    here[(f, k + 1)] = t
        states[j + 1] = here
    counts: dict[int, float] = {}
    for (_, k), t in states[-1].items():
        if t < counts.get(k, math.inf):
            counts[k] = t
    return select(counts, params.looseness)


def greedy(items: Sequence[Item], widths: Sequence[int]) -> list[int]:
    """First fit: each line takes as much as it can without being overfull.
    What most text engines do. Its lines may well be infeasible."""
    out: list[int] = []
    previous = None
    while True:
        s = line_start(items, previous)
        w = line_width(widths, len(out))
        chosen = None
        for b in range(s, len(items)):
            if not is_legal_break(items, b):
                continue
            m = measure(items, s, b)
            if m.natural - m.shrink > w and chosen is not None:
                break
            chosen = b
            if is_forced(items[b]):
                break
        assert chosen is not None, "a terminated paragraph always has a next break"
        out.append(chosen)
        if chosen == len(items) - 1:
            return out
        previous = chosen


# ---------------------------------------------------------------------------
# Checking break_lines against an oracle


class Verdict(enum.Enum):
    SOLVED = "solved"
    INFEASIBLE = "infeasible"
    #: Valid, but the optimum ties across line counts, so only validity was
    #: checked.
    AMBIGUOUS = "ambiguous"


def check(items: Sequence[Item], widths: Sequence[int], params: Params) -> Verdict:
    """Run `break_lines` and hold it to the dynamic-programming oracle."""
    return _judge(items, widths, params, oracle(items, widths, params))


def check_brute(items: Sequence[Item], widths: Sequence[int], params: Params) -> Verdict:
    """Run `break_lines` and hold it to brute force."""
    return _judge(items, widths, params, brute_force(items, widths, params))


def _judge(items: Sequence[Item], widths: Sequence[int], params: Params, want: Solution | None) -> Verdict:
    ctx = lambda: repro(items, widths, params)  # noqa: E731
    try:
        got, error = break_lines(list(items), list(widths), params), None
    except DemeritsError as e:
        got, error = None, e
    if want is None:
        if isinstance(error, Infeasible):
            return Verdict.INFEASIBLE
        raise AssertionError(
            f"no feasible solution exists, so expected Infeasible, got {repr(error) if error else got}\n{ctx()}"
        )
    if error is not None:
        raise AssertionError(f"a solution exists ({want.demerits} demerits), but got {error!r}\n{ctx()}")
    try:
        ev = validate(items, widths, params, got)
    except Invalid as e:
        raise AssertionError(f"{e}\n{ctx()}") from None
    at = [b.position for b in got]
    if want.ambiguous:
        # With looseness 0 the demerits are still known exactly.
        if params.looseness == 0 and not close(ev.demerits, want.demerits):
            raise AssertionError(
                f"not optimal: breaks at {at} cost {ev.demerits} demerits; the best costs {want.demerits}\n{ctx()}"
            )
        return Verdict.AMBIGUOUS
    if len(got) != want.lines:
        raise AssertionError(
            f"wrong number of lines (looseness {params.looseness}): got breaks at {at}, "
            f"expected {want.lines} lines\n{ctx()}"
        )
    if not close(ev.demerits, want.demerits):
        raise AssertionError(
            f"not optimal: breaks at {at} cost {ev.demerits} demerits; the best costs {want.demerits}\n{ctx()}"
        )
    return Verdict.SOLVED


def repro(items: Sequence[Item], widths: Sequence[int], params: Params) -> str:
    """Python you can paste into a test to reproduce a failure."""

    def num(x: int) -> str:
        if x == INFINITY:
            return "INFINITY"
        if x == -INFINITY:
            return "-INFINITY"
        return str(x)

    def item(it: Item) -> str:
        match it:
            case Box(width=w):
                return f"bx({num(w)})"
            case Glue(width=w, stretch=y, shrink=z) if y is INFINITE:
                return f"infinite_glue({num(w)}, {num(z)})"
            case Glue(width=w, stretch=y, shrink=z):
                return f"glue({num(w)}, {num(y)}, {num(z)})"
            case Penalty(width=w, cost=c, flagged=f):
                return f"penalty({num(w)}, {num(c)}, {f})"

    shown = f"[...]  # {len(items)} items, too many to print" if len(items) > 400 else (
        "[" + ", ".join(item(it) for it in items) + "]"
    )
    return (
        "to reproduce:\n"
        f"    items = {shown}\n"
        f"    widths = [{', '.join(num(w) for w in widths)}]\n"
        f"    params = {params!r}\n"
    )


# ---------------------------------------------------------------------------
# Random paragraphs

_M64 = (1 << 64) - 1


class Rng:
    """SplitMix64, the same generator as the Rust trials: the same seed gives
    the same numbers in both."""

    def __init__(self, seed: int) -> None:
        self.state = seed & _M64

    def next_u64(self) -> int:
        self.state = (self.state + 0x9E37_79B9_7F4A_7C15) & _M64
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58_476D_1CE4_E5B9) & _M64
        z = ((z ^ (z >> 27)) * 0x94D0_49BB_1331_11EB) & _M64
        return z ^ (z >> 31)

    def unit(self) -> float:
        """Uniform in [0, 1)."""
        return (self.next_u64() >> 11) / (1 << 53)

    def range(self, lo: float, hi: float) -> float:
        return lo + (hi - lo) * self.unit()

    def int(self, lo: int, hi: int) -> int:
        """Uniform in lo..=hi."""
        return lo + self.next_u64() % (hi - lo + 1)

    def chance(self, p: float) -> bool:
        return self.unit() < p

    def pick(self, xs: Sequence):
        return xs[self.int(0, len(xs) - 1)]


@dataclass(frozen=True)
class Shape:
    """What kind of random paragraph to make."""

    words: int = 30
    #: Chance that a word is split into hyphenated fragments.
    hyphens: float = 0.3
    #: Chance of an ordinary penalty (random cost) before a space.
    penalties: float = 0.1
    #: Chance of a forced break after a word.
    forced: float = 0.0
    #: Whether those forced breaks come with fill, like the end of a verse
    #: line.
    fills: bool = False
    #: Chance of a negative kern inside a fragment.
    kerns: float = 0.0
    #: Widths in half characters and stretch and shrink of a quarter, a half,
    #: one or two characters, so ratios land exactly on the class and
    #: tolerance boundaries, and ties are common.
    dyadic: bool = False


def paragraph(rng: Rng, shape: Shape) -> list[Item]:
    items: list[Item] = []
    space_next = False
    for w in range(shape.words):
        if space_next:
            if rng.chance(shape.penalties):
                items.append(penalty(0, rng.int(-200, 300), False))
            if shape.dyadic:
                items.append(glue(CH, rng.pick([CH // 2, CH, 2 * CH]), rng.pick([CH // 4, CH // 2, CH])))
            else:
                items.append(glue(rng.int(800, 1200), rng.int(200, 1000), rng.int(100, 500)))
        fragments = rng.int(2, 3) if rng.chance(shape.hyphens) else 1
        for f in range(fragments):
            if f > 0:
                items.append(HYPHEN)
            width = rng.int(3, 12) * CH // 2 if shape.dyadic else rng.int(1500, 6000)
            if rng.chance(shape.kerns):
                # A kern inside the fragment; the fragment's net width stays
                # at least 1.2 characters, wider than a hyphen.
                kern = rng.int(50, 300)
                left = rng.int(500, width - 500)
                items.extend([bx(left), bx(-kern), bx(width - left)])
            else:
                items.append(bx(width))
        space_next = True
        if w + 1 < shape.words and rng.chance(shape.forced):
            if shape.fills:
                items.extend([forbidden(), fill()])
            items.append(forced())
            space_next = False
    finish(items)
    return items


def random_params(rng: Rng) -> Params:
    """Random parameters with looseness 0."""
    tolerance = rng.pick([1.0, 1.5, 2.0, 3.0, 5.0, 10.0])
    line_penalty = float(rng.int(0, 20))
    flagged = rng.pick([0.0, 100.0, 3000.0, 10_000.0])
    fitness_ = rng.pick([0.0, 100.0, 3000.0, 10_000.0])
    return Params(tolerance, line_penalty, flagged, fitness_, 0)


def random_widths(rng: Rng, parshape: bool, dyadic: bool, lo: int, hi: int) -> list[int]:
    """One line width between `lo` and `hi` characters, or (with `parshape`)
    one to four."""
    count = rng.int(1, 4) if parshape else 1
    return [rng.int(lo * 2, hi * 2) * CH // 2 if dyadic else rng.int(lo * CH, hi * CH) for _ in range(count)]
