# demerits

A Rust based, deterministic Knuth-Plass typesetter.

`demerits` breaks a paragraph into lines the way TeX does: it looks at the
whole paragraph and picks the breaks with the fewest total demerits, instead
of filling each line greedily. The same input gives the same lines, bit for
bit, on every platform.

For when the best isn't needed, it also has two first-fit breakers, which
fill each line as full as it will go and never fail:

- `greedy` justifies: a line fits if its glue can shrink enough.
- `ragged` doesn't: a line fits at its natural width, as in ragged-right text
  or an editor wrapping lines.

```rust
let breaks = demerits::ragged(&items, &[line_width])?;
```

The full contract (integer widths, what counts as a break, how lines are
measured, the adjustment ratio, demerits, looseness, first fit) is in the
crate docs at the top of [`src/lib.rs`](src/lib.rs).

## The trials

The tests live in [`trials/`](trials), an unpublished crate, in six levels:

| Level | File | What it asks |
|---|---|---|
| easy | `tests/easy.rs` | Does it break lines at all? |
| medium | `tests/medium.rs` | Is it really Knuth–Plass, or greedy in a nice coat? |
| hard | `tests/hard.rs` | Hyphens, fitness classes, line shapes and looseness. |
| very hard | `tests/very_hard.rs` | Thousands of random paragraphs against the oracle. |
| nitpicking | `tests/nitpicking.rs` | Every rule in the contract, at its edges. |
| neptune | `tests/neptune.rs` | You have angered Neptune. |

Run the current level:

```sh
cargo test
```

Each level includes the ones below it. The current level is `default` in
[`trials/Cargo.toml`](trials/Cargo.toml); raise it there when you pass. To
peek at a harder level without committing to it:

```sh
cargo test -p trials --features hard
```

A failing check prints Rust you can paste into a test to reproduce it.

`greedy` and `ragged` are finished, so their trials, in
[`tests/first_fit.rs`](trials/tests/first_fit.rs), aren't a level: they
always run. To run only them:

```sh
cargo test -p trials --test first_fit
```

The judges in `trials/src/lib.rs` are written from the contract, not from
an implementation: a line-by-line scorer, brute force over every set of breaks
for small paragraphs, and an exhaustive dynamic program for bigger ones. The
dynamic program is itself checked against brute force on every run. First fit
is judged by a slow version that measures every line from scratch, and by
first fit's promises checked line by line.

## Sketching in Python

[`python/`](python) has the same interface in Python, for getting the
algorithm right before writing it in Rust: a stub in `demerits.py`, and easy
and medium ported to `unittest`, with the same judges and the same random
paragraphs (the generator is the same SplitMix64, so a seed makes the same
paragraph in both). It needs nothing but Python 3.10 or later.

```sh
python3 python/run.py           # the current level
python3 python/run.py medium    # peek at the next
```

Raise `LEVEL` at the top of `python/run.py` when you pass. The Python
differences from the contract (exceptions instead of `Err`, `INFINITE` for
infinite stretch) are at the top of `demerits.py`.

## License

MIT
