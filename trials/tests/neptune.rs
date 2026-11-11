//! You have angered Neptune, vengeful and capricious god of the sea. Enjoy
//! your trip, Odysseus.
#![cfg(feature = "neptune")]

use std::time::Duration;
use trials::*;

fn p() -> Params {
    Params::default()
}

fn positions(r: Result<Vec<Break>, Error>) -> Result<Vec<usize>, Error> {
    r.map(|bs| bs.iter().map(|b| b.position).collect())
}

// --- Scylla: infinity in the middle of the paragraph

#[test]
fn verse_lines() {
    // Every verse ends in a fill and a forced break, so infinite stretch turns
    // up again and again. Infinity minus infinity is not zero.
    let items = text(PSALM_23);
    for width in [30, 40, 45, 60, 90, 200] {
        assert_ne!(check(&items, &[width * CH], &Params { tolerance: 3.0, ..p() }), Verdict::Infeasible, "at width {width}");
    }
}

#[test]
fn a_fill_in_the_middle_of_a_line() {
    // "Chapter One ........ 1" as the first line, then a paragraph.
    let mut items = vec![bx(7 * CH), SPACE, bx(3 * CH), infinite_glue(CH, 0), bx(CH), forced()];
    items.extend(text(CONFESSIONS));
    for width in 18..=40 {
        check(&items, &[width * CH], &Params { tolerance: 3.0, ..p() });
    }
}

#[test]
fn fills_everywhere() {
    let mut rng = Rng::new(0x5ea);
    for case in 0..500 {
        let shape = Shape { words: rng.int(10, 50) as usize, forced: 0.15, fills: true, ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        let widths = random_widths(&mut rng, case % 3 == 0, false, 12, 35);
        check(&items, &widths, &random_params(&mut rng));
    }
}

#[test]
fn an_overfull_line_with_a_fill_still_shrinks() {
    // Infinite stretch is no help to a line that's too long.
    let items = [bx(4), glue(1, 0, 1), bx(4), forbidden(), fill(), forced()];
    assert_eq!(break_lines(&items, &[8], &p()), Ok(vec![Break { position: 5, ratio: -1.0 }]));
}

// --- Charybdis: paragraphs that are barely there

#[test]
fn an_empty_paragraph_with_its_fill() {
    assert_eq!(break_lines(&[forbidden(), fill(), forced()], &[10], &p()), Ok(vec![Break { position: 2, ratio: 0.0 }]));
}

#[test]
fn a_paragraph_of_nothing_but_glue() {
    let items = [glue(1, 1, 1), glue(1, 1, 1), forced()];
    assert_eq!(break_lines(&items, &[2], &p()), Ok(vec![Break { position: 2, ratio: 0.0 }]));
    assert_eq!(break_lines(&items, &[3], &p()), Ok(vec![Break { position: 2, ratio: 0.5 }]));
    assert_eq!(break_lines(&items, &[1], &p()), Ok(vec![Break { position: 2, ratio: -0.5 }]));
}

#[test]
fn a_forced_break_at_the_very_start() {
    // The first line is empty and has nothing to stretch.
    let items = [forced(), bx(3), forbidden(), fill(), forced()];
    assert_eq!(break_lines(&items, &[10], &p()), Err(Error::Infeasible));
}

#[test]
fn a_blank_line_needs_a_box_to_hold_it_open() {
    // After a break, the fill is discarded along with everything else up to
    // the next box or forced break, so a blank line has nothing to stretch.
    let mut bare = text("one /");
    bare.extend([forbidden(), fill(), forced()]);
    bare.extend(text("two"));
    assert_eq!(break_lines(&bare, &[10 * CH], &p()), Err(Error::Infeasible));

    let mut propped = text("one /");
    propped.extend([bx(0), forbidden(), fill(), forced()]);
    propped.extend(text("two"));
    assert_eq!(positions(break_lines(&propped, &[10 * CH], &p())), Ok(vec![3, 7, 11]));
}

#[test]
fn zero_width_everything() {
    let items = [bx(0), glue(0, 0, 0), bx(0), forced()];
    assert_eq!(break_lines(&items, &[0], &p()), Ok(vec![Break { position: 3, ratio: 0.0 }]));
}

#[test]
fn a_word_wider_than_the_column() {
    let items = text("and then Pneumonoultramicroscopicsilicovolcanoconiosis struck");
    assert_eq!(break_lines(&items, &[30 * CH], &p()), Err(Error::Infeasible));
}

#[test]
fn an_absurdly_wide_column() {
    // The widest column there is. Subtract from it all you like; add to it
    // and you overflow.
    let items = text(CONFESSIONS);
    assert_eq!(
        break_lines(&items, &[i64::MAX], &p()),
        Ok(vec![Break { position: items.len() - 1, ratio: 0.0 }])
    );
    // And again on the lines after the first, where you've already measured
    // something: one verse to a line.
    let verses = text(PSALM_23);
    let ends: Vec<usize> = (0..verses.len()).filter(|&i| is_forced(&verses[i])).collect();
    assert_eq!(positions(break_lines(&verses, &[i64::MAX], &p())), Ok(ends));
}

// --- The Sirens: numbers that sing you onto the rocks

#[test]
fn scale_is_just_scale() {
    // Multiply every width by the same integer and nothing may change: not
    // one break, not one bit of one ratio. The odd factors keep every sum
    // under 2^53; the powers of two go past it.
    let mut rng = Rng::new(0x5ca1e);
    for _ in 0..150 {
        let shape = Shape { words: rng.int(10, 60) as usize, forced: 0.05, fills: rng.chance(0.5), ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        let parshape = rng.chance(0.3);
        let widths = random_widths(&mut rng, parshape, false, 12, 35);
        let params = random_params(&mut rng);
        let base = break_lines(&items, &widths, &params);
        for k in [3, 1000, 7919, 1 << 20, 999_999_937, 1 << 40] {
            let s = |x: i64| x.checked_mul(k).expect("the test's own scaling overflowed");
            let scaled_items: Vec<Item> = items
                .iter()
                .map(|it| match *it {
                    Item::Box { width } => bx(s(width)),
                    Item::Glue { width, stretch: Stretch::Finite(y), shrink } => glue(s(width), s(y), s(shrink)),
                    Item::Glue { width, stretch: Stretch::Infinite, shrink } => infinite_glue(s(width), s(shrink)),
                    Item::Penalty { width, cost, flagged } => penalty(s(width), cost, flagged),
                })
                .collect();
            let scaled_widths: Vec<i64> = widths.iter().map(|&w| s(w)).collect();
            assert_eq!(
                break_lines(&scaled_items, &scaled_widths, &params),
                base,
                "scaled by {k}\n{}",
                repro(&items, &widths, &params)
            );
        }
    }
}

#[test]
fn past_two_to_the_fifty_three() {
    // An f64 holds every integer up to 2^53, then starts skipping them. Your
    // sums don't get to skip.
    let big = 1i64 << 60;
    // One unit too wide, with nothing to shrink. In f64, 2^60 + 1 is 2^60,
    // and it fits.
    assert_eq!(break_lines(&[bx(big), bx(1), forced()], &[big], &p()), Err(Error::Infeasible));
    // Two units short, with one unit of stretch: a ratio of exactly 2. In f64
    // the line is already full.
    let items = [bx(big), glue(0, 1, 0), bx(1), forced()];
    assert_eq!(break_lines(&items, &[big + 3], &p()), Ok(vec![Break { position: 3, ratio: 2.0 }]));
}

#[test]
fn a_huge_line_and_then_ordinary_ones() {
    // Running totals in f64 would forget everything after the first line.
    let big = 1i64 << 60;
    let mut items = vec![bx(big), forced()];
    items.extend(text(CONFESSIONS));
    for width in [30, 40, 50] {
        assert_ne!(check(&items, &[big, width * CH], &p()), Verdict::Infeasible, "at width {width}");
    }
}

#[test]
fn kerns_pull_backwards() {
    // Negative boxes inside words. Still optimal, because each word's net
    // width stays positive.
    let mut rng = Rng::new(0x6e6);
    for case in 0..300 {
        let shape = Shape { words: rng.int(8, 35) as usize, kerns: 0.4, hyphens: 0.4, ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        let widths = random_widths(&mut rng, case % 2 == 0, false, 10, 30);
        check(&items, &widths, &random_params(&mut rng));
    }
}

#[test]
fn zero_tolerance() {
    // Lines may only shrink.
    let mut rng = Rng::new(0);
    for _ in 0..300 {
        let words = rng.int(5, 30) as usize;
        let items = paragraph(&mut rng, Shape { words, hyphens: 0.6, ..Shape::default() });
        let widths = random_widths(&mut rng, false, false, 10, 25);
        check(&items, &widths, &Params { tolerance: 0.0, ..random_params(&mut rng) });
    }
}

#[test]
fn absurd_looseness() {
    let items = text(&hyphenate(CONFESSIONS));
    for looseness in [i32::MIN, -1000, 1000, i32::MAX] {
        for width in [20, 30, 45] {
            check(&items, &[width * CH], &Params { looseness, tolerance: 10.0, ..p() });
        }
    }
}

// --- Poseidon himself

const MIB: usize = 1024 * 1024;

#[test]
fn a_hundred_thousand_lines_on_a_small_stack() {
    // One word per line, a hundred thousand times, on a thread with 256 KiB
    // of stack. Walk back through your breaks however you like, as long as
    // it isn't recursion. (Overflow the stack and every Neptune test dies
    // with you.)
    let items = text(&vec!["a"; 100_000].join(" "));
    let breaks = within(Duration::from_secs(5), MIB / 4, "100,000 lines", {
        let items = items.clone();
        move || break_lines(&items, &[CH], &p())
    })
    .unwrap();
    assert_eq!(breaks.len(), 100_000);
    assert!(breaks.iter().all(|b| b.ratio == 0.0));
}

#[test]
fn fifty_thousand_words() {
    let mut rng = Rng::new(50_000);
    let items = paragraph(&mut rng, Shape { words: 50_000, hyphens: 0.3, penalties: 0.05, ..Shape::default() });
    let breaks = within(Duration::from_secs(3), 8 * MIB, "50,000 words", {
        let items = items.clone();
        move || break_lines(&items, &[60 * CH], &p())
    })
    .unwrap();
    validate(&items, &[60 * CH], &p(), &breaks).unwrap();
}

#[test]
fn a_tolerance_that_forgives_everything() {
    // Nearly every line is feasible, so every break is a candidate for every
    // line number. You don't need to tell those apart when every line is the
    // same width and nobody asked for looseness.
    let mut rng = Rng::new(10);
    let items = paragraph(&mut rng, Shape { words: 5_000, hyphens: 0.3, ..Shape::default() });
    let params = Params { tolerance: 1e6, ..p() };
    let breaks = within(Duration::from_secs(3), 8 * MIB, "5,000 forgiving words", {
        let items = items.clone();
        move || break_lines(&items, &[60 * CH], &params)
    })
    .unwrap();
    validate(&items, &[60 * CH], &params, &breaks).unwrap();
}
