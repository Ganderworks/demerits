# demerits

A Rust based, deterministic Knuth-Plass typesetter.

`demerits` breaks a paragraph into lines the way TeX does: it looks at the
whole paragraph and picks the breaks with the fewest total demerits, instead
of filling each line greedily. The same input gives the same lines, bit for
bit, on every platform.

The full contract (integer widths, what counts as a break, how lines are
measured, the adjustment ratio, demerits, looseness) is in the crate docs at
the top of [`src/lib.rs`](src/lib.rs).

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

The judges in `trials/src/lib.rs` are written from the contract, not from
an implementation: a line-by-line scorer, brute force over every set of breaks
for small paragraphs, and an exhaustive dynamic program for bigger ones. The
dynamic program is itself checked against brute force on every run.

## License

MIT
