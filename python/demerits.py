"""Deterministic Knuth–Plass line breaking: the Python sketch.

The same interface as the Rust crate, for getting the algorithm right before
writing it in Rust. The full contract is in the crate docs at the top of
`src/lib.rs`, and everything there holds here, with these differences:

- Errors are exceptions: `break_lines` raises `Infeasible` or `Unterminated`
  instead of returning `Err`.
- Infinite stretch is the `INFINITE` sentinel in place of `Stretch::Infinite`.
  A finite stretch is a plain `int`.
- Python's ints don't overflow, so the promises about `i64` don't apply.
- The ratio has to be `float(W - L) / float(Y)` (or `Z`). Plain `(W - L) / Y`
  rounds once from the exact quotient, while Rust converts each side first,
  and past 2**53 the two can differ in the last bit.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass

#: A penalty cost at or above this never breaks; at or below its negative,
#: always breaks.
INFINITY = 10_000


class _Infinite:
    """The type of `INFINITE`. There is only one."""

    __slots__ = ()

    def __repr__(self) -> str:
        return "INFINITE"


#: Glue stretch without limit, like TeX's `\hfil`. A line with any of it
#: never has a positive ratio.
INFINITE = _Infinite()


@dataclass(frozen=True, slots=True)
class Box:
    """Something with a fixed width: a word, a syllable, a kern."""

    width: int


@dataclass(frozen=True, slots=True)
class Glue:
    """Space that can stretch and shrink. `stretch` is an int or `INFINITE`."""

    width: int
    stretch: int | _Infinite
    shrink: int


@dataclass(frozen=True, slots=True)
class Penalty:
    """A possible break. `width` is only used if the line breaks here (a
    hyphen, say). `flagged` marks hyphen-like breaks."""

    width: int
    cost: int
    flagged: bool


Item = Box | Glue | Penalty


@dataclass(frozen=True, slots=True)
class Params:
    """How to judge a paragraph."""

    #: The largest adjustment ratio a line may have.
    tolerance: float = 2.0
    #: Added to every line's badness before squaring: the paper's `l`.
    line_penalty: float = 10.0
    #: Charged when two lines in a row end at flagged penalties: the paper's α.
    flagged_demerits: float = 3000.0
    #: Charged when neighbouring lines' fitness classes differ by more than
    #: one: the paper's γ.
    fitness_demerits: float = 3000.0
    #: 0 for the optimum; +1 asks for one more line, -1 for one fewer.
    looseness: int = 0


@dataclass(frozen=True, slots=True)
class Break:
    """Where a line ends, and how much its glue is stretched (`ratio > 0`) or
    shrunk (`ratio < 0`)."""

    #: The index of the item the line breaks at.
    position: int
    #: The line's adjustment ratio.
    ratio: float


class DemeritsError(Exception):
    """Why there's no answer."""


class Infeasible(DemeritsError):
    """No set of breaks makes every line feasible."""


class Unterminated(DemeritsError):
    """The paragraph doesn't end with a forced break."""


def break_lines(items: Sequence[Item], line_widths: Sequence[int], params: Params) -> list[Break]:
    """Break a paragraph into lines.

    Returns one `Break` per line, in order; the last is the paragraph's final
    forced break. An empty paragraph has no lines. Raises `Infeasible` or
    `Unterminated`, and something (anything) if `line_widths` is empty and
    `items` isn't.
    """
    raise NotImplementedError("this one's yours")
