//! First fit: `greedy` and `ragged`. These are finished breakers, not
//! levels, so their trials always run, and they're held to the whole
//! contract at once. Most trials run both; the rules are the same apart
//! from what "fits" means.

use std::sync::Arc;
use std::time::Duration;
use trials::*;

const MIB: usize = 1024 * 1024;

fn positions(breaks: &[Break]) -> Vec<usize> {
    breaks.iter().map(|b| b.position).collect()
}

fn ratios(breaks: &[Break]) -> Vec<f64> {
    breaks.iter().map(|b| b.ratio).collect()
}

/// Bit for bit, so `-0.0` isn't `0.0`.
fn same_bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// Check everything, then return the break positions.
fn at(fit: Fit, items: &[Item], widths: &[i64]) -> Vec<usize> {
    positions(&check_first_fit(items, widths, fit))
}

// --- the shape of an answer

#[test]
fn an_empty_paragraph_has_no_lines() {
    for fit in Fit::BOTH {
        assert_eq!(fit.run(&[], &[10]), Ok(vec![]), "{fit:?}");
        assert_eq!(fit.run(&[], &[]), Ok(vec![]), "{fit:?}: no items, so no widths needed");
    }
}

#[test]
fn a_paragraph_must_end_with_a_forced_break() {
    for fit in Fit::BOTH {
        for items in [
            vec![bx(3)],
            vec![bx(3), glue(1, 1, 1)],
            vec![bx(3), penalty(0, 0, false)],
            vec![bx(3), penalty(0, -INFINITY + 1, false)],
            vec![bx(3), forced(), bx(3)],
            vec![forced(), forbidden()],
        ] {
            assert_eq!(fit.run(&items, &[10]), Err(Error::Unterminated), "{fit:?}: {items:?}");
        }
    }
}

#[test]
fn no_line_widths_is_a_bug_in_the_caller() {
    for fit in Fit::BOTH {
        for items in [vec![bx(3), forced()], vec![forced()]] {
            let got = std::panic::catch_unwind(|| fit.run(&items, &[]));
            assert!(got.is_err(), "{fit:?} should panic with no line widths, but returned {got:?}");
        }
    }
}

#[test]
fn a_short_paragraph_is_one_line() {
    let items = text("Great art Thou, O Lord");
    for fit in Fit::BOTH {
        assert_eq!(fit.run(&items, &[40 * CH]), Ok(vec![Break { position: items.len() - 1, ratio: 0.0 }]), "{fit:?}");
    }
}

#[test]
fn an_exact_fit_is_a_fit() {
    for fit in Fit::BOTH {
        assert_eq!(at(fit, &[bx(10), forced()], &[10]), [1], "{fit:?}");
        // "aaa bbb" is exactly seven characters.
        assert_eq!(at(fit, &text("aaa bbb"), &[7 * CH]), [5], "{fit:?}");
    }
}

// --- where a line may break

#[test]
fn a_forced_break_is_always_taken() {
    for fit in Fit::BOTH {
        for cost in [-INFINITY, -INFINITY - 1, i64::MIN] {
            let items = [bx(3), penalty(0, cost, false), bx(3), forced()];
            assert_eq!(at(fit, &items, &[100]), [1, 3], "{fit:?}, cost {cost}");
        }
    }
}

#[test]
fn a_cost_of_infinity_never_breaks() {
    for fit in Fit::BOTH {
        for cost in [INFINITY, INFINITY + 1, i64::MAX] {
            // Even when the line is far too long without it.
            let items = [bx(5), penalty(0, cost, false), bx(5), forced()];
            assert_eq!(at(fit, &items, &[5]), [3], "{fit:?}, cost {cost}");
        }
        let items = [bx(5), penalty(0, INFINITY - 1, false), bx(5), forced()];
        assert_eq!(at(fit, &items, &[5]), [1, 3], "{fit:?}");
    }
}

#[test]
fn finite_costs_make_no_difference() {
    let mut rng = Rng::new(0xc057);
    for _ in 0..300 {
        let words = rng.int(5, 40) as usize;
        let items = paragraph(&mut rng, Shape { words, penalties: 0.5, ..Shape::default() });
        let widths = random_widths(&mut rng, true, false, 5, 30);
        let recost: Vec<Item> = items
            .iter()
            .map(|it| match *it {
                Item::Penalty { width, cost, .. } if cost.abs() < INFINITY => penalty(width, rng.int(-9999, 9999), rng.chance(0.5)),
                other => other,
            })
            .collect();
        for fit in Fit::BOTH {
            assert_eq!(fit.run(&items, &widths), fit.run(&recost, &widths), "{fit:?}\n{}", repro_layout(&items, &widths));
        }
    }
}

#[test]
fn glue_is_a_break_only_after_a_box() {
    for fit in Fit::BOTH {
        // Glue at the start isn't a break, and it takes up room.
        let items = [glue(2, 0, 0), bx(3), glue(1, 0, 0), bx(3), forced()];
        assert_eq!(at(fit, &items, &[1]), [2, 4], "{fit:?}: glue at the start");
        assert_eq!(at(fit, &items, &[7]), [2, 4], "{fit:?}: glue at the start");
        // Glue after glue.
        let items = [bx(3), glue(1, 0, 0), glue(1, 0, 0), bx(3), forced()];
        assert_eq!(at(fit, &items, &[4]), [1, 4], "{fit:?}: glue after glue");
        // Glue after a penalty.
        let items = [bx(3), penalty(0, 0, false), glue(1, 0, 0), bx(3), forced()];
        assert_eq!(at(fit, &items, &[4]), [1, 4], "{fit:?}: glue after a penalty");
    }
}

// --- what a line contains

#[test]
fn the_space_at_a_break_takes_no_room() {
    for fit in Fit::BOTH {
        assert_eq!(fit.run(&text("aaa bbb"), &[3 * CH]), Ok(vec![Break { position: 1, ratio: 0.0 }, Break { position: 5, ratio: 0.0 }]), "{fit:?}");
    }
}

#[test]
fn glue_and_penalties_after_a_break_are_discarded() {
    for fit in Fit::BOTH {
        let items = [bx(4), glue(1, 0, 0), penalty(0, 0, false), glue(1, 0, 0), bx(4), forced()];
        assert_eq!(at(fit, &items, &[4]), [1, 5], "{fit:?}");
    }
}

#[test]
fn no_line_ends_inside_a_discarded_run() {
    // An empty line at index 2 would fit the second width exactly, but it's
    // in the run discarded after the break at 1.
    //   0 B4, 1 G, 2 P, 3 G, 4 B0, 5 G0, 6 B4, 7..9 finish
    for fit in Fit::BOTH {
        for hyphen in [0, 1] {
            let items = [
                bx(4),
                glue(1, 0, 0),
                penalty(hyphen, -100, false),
                glue(1, 0, 0),
                bx(0),
                glue(0, 0, 0),
                bx(4),
                forbidden(),
                fill(),
                forced(),
            ];
            assert_eq!(at(fit, &items, &[4, 0, 4]), [1, 5, 9], "{fit:?}, penalty width {hyphen}");
        }
    }
}

#[test]
fn a_penalty_takes_up_room_only_where_the_line_breaks() {
    //   0 B2, 1 G, 2 B3, 3 P(2), 4 B3, 5..7 finish
    // Broken at 3, the first line is 2 + 1 + 3 + 2 = 8: too long for 6.
    // Not broken at 3, the second line is 3 + 3 = 6: just right.
    let items = [bx(2), glue(1, 0, 0), bx(3), penalty(2, 0, true), bx(3), forbidden(), fill(), forced()];
    for fit in Fit::BOTH {
        assert_eq!(at(fit, &items, &[6]), [1, 7], "{fit:?}");
        // One unit wider and the hyphen fits.
        assert_eq!(at(fit, &items, &[8]), [3, 7], "{fit:?}");
    }
}

#[test]
fn infinite_glue_still_has_its_natural_width() {
    let items = [bx(3), glue(1, 0, 0), bx(3), forbidden(), infinite_glue(4, 0), bx(1), forced()];
    for fit in Fit::BOTH {
        assert_eq!(at(fit, &items, &[8]), [1, 6], "{fit:?}");
        assert_eq!(at(fit, &items, &[12]), [6], "{fit:?}");
    }
}

#[test]
fn each_line_gets_its_own_width() {
    let items = text("aa bb cc dd ee");
    for fit in Fit::BOTH {
        // Two characters, then five, and five for every line after.
        assert_eq!(at(fit, &items, &[2 * CH, 5 * CH]), [1, 5, 11], "{fit:?}");
    }
}

#[test]
fn repeating_the_last_width_changes_nothing() {
    let mut rng = Rng::new(0x1a57);
    for _ in 0..200 {
        let words = rng.int(5, 60) as usize;
        let items = paragraph(&mut rng, Shape { words, forced: 0.05, ..Shape::default() });
        let widths = random_widths(&mut rng, true, false, 8, 30);
        let mut longer = widths.clone();
        longer.extend([*widths.last().unwrap(); 5]);
        for fit in Fit::BOTH {
            assert_eq!(fit.run(&items, &widths), fit.run(&items, &longer), "{fit:?}\n{}", repro_layout(&items, &widths));
        }
    }
}

// --- lines that are too long

#[test]
fn a_word_wider_than_the_column_gets_a_line_of_its_own() {
    let items = text("and then Pneumonoultramicroscopicsilicovolcanoconiosis struck");
    for fit in Fit::BOTH {
        assert_eq!(at(fit, &items, &[30 * CH]), [3, 5, 9], "{fit:?}");
    }
    let got = greedy(&items, &[30 * CH]).unwrap();
    assert!(same_bits(&ratios(&got), &[44.0, f64::NEG_INFINITY, 0.0]), "{got:?}");

    let items = text("Pneumonoultramicroscopicsilicovolcanoconiosis Pneumonoultramicroscopicsilicovolcanoconiosis a");
    for fit in Fit::BOTH {
        assert_eq!(at(fit, &items, &[10 * CH]), [1, 3, 7], "{fit:?}");
    }
}

#[test]
fn a_column_nothing_fits_in() {
    // Every line is too long at its first break, so every break is taken.
    let items = text(CONFESSIONS);
    let every: Vec<usize> = (0..items.len()).filter(|&i| is_legal_break(&items, i)).collect();
    for fit in Fit::BOTH {
        for width in [0, -5 * CH] {
            assert_eq!(at(fit, &items, &[width]), every, "{fit:?} at {width}");
        }
    }
}

#[test]
fn a_line_stops_at_the_first_break_that_is_too_long() {
    for fit in Fit::BOTH {
        // Too long at 3, though the negative box would make 6 fit again.
        //   0 B6, 1 G, 2 B6, 3 G, 4 B6, 5 B-12, 6 G, 7 B1, 8 forced
        let items = [bx(6), glue(1, 0, 0), bx(6), glue(1, 0, 0), bx(6), bx(-12), glue(1, 0, 0), bx(1), forced()];
        assert_eq!(at(fit, &items, &[10]), [1, 8], "{fit:?}");
        // Too long at the first break, though 3 and 5 would fit: the line
        // ends at 1, too long, and doesn't go looking.
        let items = [bx(12), glue(1, 0, 0), bx(-8), glue(1, 0, 0), bx(1), forced()];
        assert_eq!(at(fit, &items, &[10]), [1, 5], "{fit:?}");
    }
}

#[test]
fn a_forced_break_that_does_not_fit_waits_for_the_next_line() {
    for fit in Fit::BOTH {
        // "aaa bbb" then a forced break, in a column of five: the forced break
        // ends the second line, not the first.
        let items = [bx(3), glue(1, 0, 0), bx(3), forced(), bx(3), forced()];
        assert_eq!(at(fit, &items, &[5]), [1, 3, 5], "{fit:?}");
        // And it's taken at once when it's the first break there is.
        let items = [bx(9), forced(), bx(3), forced()];
        assert_eq!(at(fit, &items, &[5]), [1, 3], "{fit:?}");
    }
}

// --- lines that are barely there

#[test]
fn blank_lines() {
    let mut items = text("one /");
    items.push(forced());
    items.extend(text("two"));
    for fit in Fit::BOTH {
        assert_eq!(at(fit, &items, &[10 * CH]), [3, 4, 8], "{fit:?}");
    }
    // The blank line has nothing to stretch.
    let got = greedy(&items, &[10 * CH]).unwrap();
    assert!(same_bits(&ratios(&got), &[0.0, f64::INFINITY, 0.0]), "{got:?}");
}

#[test]
fn a_forced_break_at_the_very_start() {
    let items = [forced(), bx(3), forbidden(), fill(), forced()];
    for fit in Fit::BOTH {
        assert_eq!(at(fit, &items, &[10]), [0, 4], "{fit:?}");
    }
    assert_eq!(greedy(&[forced()], &[10]), Ok(vec![Break { position: 0, ratio: f64::INFINITY }]));
    assert_eq!(greedy(&[forced()], &[0]), Ok(vec![Break { position: 0, ratio: 0.0 }]));
    assert_eq!(greedy(&[forced()], &[-1]), Ok(vec![Break { position: 0, ratio: f64::NEG_INFINITY }]));
    for width in [10, 0, -1] {
        assert_eq!(ragged(&[forced()], &[width]), Ok(vec![Break { position: 0, ratio: 0.0 }]));
    }
}

#[test]
fn a_paragraph_of_nothing_but_glue() {
    let items = [glue(1, 1, 1), glue(1, 1, 1), forced()];
    for (width, r) in [(2, 0.0), (3, 0.5), (1, -0.5)] {
        assert_eq!(greedy(&items, &[width]), Ok(vec![Break { position: 2, ratio: r }]), "at {width}");
        assert_eq!(ragged(&items, &[width]), Ok(vec![Break { position: 2, ratio: 0.0 }]), "at {width}");
    }
}

#[test]
fn zero_width_everything() {
    let items = [bx(0), glue(0, 0, 0), bx(0), forced()];
    for fit in Fit::BOTH {
        let got = fit.run(&items, &[0]).unwrap();
        assert_eq!(positions(&got), [3], "{fit:?}");
        assert!(same_bits(&ratios(&got), &[0.0]), "{fit:?}: {got:?}");
    }
}

// --- big numbers

#[test]
fn the_widest_column_there_is() {
    let items = text(CONFESSIONS);
    let verses = text(PSALM_23);
    let ends: Vec<usize> = (0..verses.len()).filter(|&i| is_forced(&verses[i])).collect();
    for fit in Fit::BOTH {
        assert_eq!(fit.run(&items, &[i64::MAX]), Ok(vec![Break { position: items.len() - 1, ratio: 0.0 }]), "{fit:?}");
        assert_eq!(at(fit, &verses, &[i64::MAX]), ends, "{fit:?}");
    }
}

#[test]
fn past_two_to_the_fifty_three() {
    // In f64, 2^60 + 1 is 2^60. Not here.
    let big = 1i64 << 60;
    for fit in Fit::BOTH {
        assert_eq!(at(fit, &[bx(big), glue(0, 0, 0), bx(1), forced()], &[big]), [1, 3], "{fit:?}");
        assert_eq!(at(fit, &[bx(big), glue(0, 0, 1), bx(2), forced()], &[big]), [1, 3], "{fit:?}");
    }
    // One unit over, one unit of shrink: a ratio of exactly -1, which fits.
    assert_eq!(greedy(&[bx(big), glue(0, 0, 1), bx(1), forced()], &[big]), Ok(vec![Break { position: 3, ratio: -1.0 }]));
    assert_eq!(ragged(&[bx(big), glue(0, 0, 1), bx(1), forced()], &[big]).map(|b| positions(&b)), Ok(vec![1, 3]));
    // Two units short, one unit of stretch: exactly 2.
    assert_eq!(greedy(&[bx(big), glue(0, 1, 0), bx(1), forced()], &[big + 3]), Ok(vec![Break { position: 3, ratio: 2.0 }]));
}

#[test]
fn a_huge_line_and_then_ordinary_ones() {
    // Running totals in f64 would forget everything after the first line.
    let big = 1i64 << 60;
    let body = text(CONFESSIONS);
    let mut items = vec![bx(big), forced()];
    items.extend(&body);
    for fit in Fit::BOTH {
        for width in [20, 30, 45] {
            let alone = at(fit, &body, &[width * CH]);
            let after = at(fit, &items, &[big, width * CH]);
            let shifted: Vec<usize> = alone.iter().map(|b| b + 2).collect();
            assert_eq!(after[1..], shifted[..], "{fit:?} at {width}");
        }
    }
}

#[test]
fn scale_is_just_scale() {
    // Multiply every width by the same integer: not one break, not one bit
    // of one ratio may change.
    let mut rng = Rng::new(0xf17_5ca1e);
    for _ in 0..150 {
        let shape = Shape { words: rng.int(10, 60) as usize, forced: 0.05, fills: rng.chance(0.5), kerns: 0.2, ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        let parshape = rng.chance(0.3);
        let widths = random_widths(&mut rng, parshape, false, 5, 35);
        for fit in Fit::BOTH {
            let base = fit.run(&items, &widths).unwrap();
            for k in [3, 1000, 7919, 1 << 20, 999_999_937, 1 << 40] {
                let s = |x: i64| x.checked_mul(k).expect("the test's own scaling overflowed");
                let scaled: Vec<Item> = items
                    .iter()
                    .map(|it| match *it {
                        Item::Box { width } => bx(s(width)),
                        Item::Glue { width, stretch: Stretch::Finite(y), shrink } => glue(s(width), s(y), s(shrink)),
                        Item::Glue { width, stretch: Stretch::Infinite, shrink } => infinite_glue(s(width), s(shrink)),
                        Item::Penalty { width, cost, flagged } => penalty(s(width), cost, flagged),
                    })
                    .collect();
                let widths_k: Vec<i64> = widths.iter().map(|&w| s(w)).collect();
                let got = fit.run(&scaled, &widths_k).unwrap();
                assert!(
                    positions(&got) == positions(&base) && same_bits(&ratios(&got), &ratios(&base)),
                    "{fit:?} scaled by {k}: {got:?} vs {base:?}\n{}",
                    repro_layout(&items, &widths)
                );
            }
        }
    }
}

// --- greedy

#[test]
fn greedy_shrinks_a_line_to_fit_and_ragged_does_not() {
    // "aaa bbb" is seven characters; the space can give up a quarter.
    let items = text("aaa bbb");
    assert_eq!(greedy(&items, &[7 * CH - 1]), Ok(vec![Break { position: 5, ratio: -1.0 / 250.0 }]));
    assert_eq!(ragged(&items, &[7 * CH - 1]).map(|b| positions(&b)), Ok(vec![1, 5]));
}

#[test]
fn greedy_a_ratio_of_exactly_minus_one_fits() {
    let items = [bx(4), glue(1, 0, 1), bx(4), forbidden(), fill(), forced()];
    assert_eq!(greedy(&items, &[8]), Ok(vec![Break { position: 5, ratio: -1.0 }]));
    // A fill doesn't help a line that's too long.
    let got = greedy(&items, &[7]).unwrap();
    assert_eq!(positions(&got), [1, 5]);
    assert!(same_bits(&ratios(&got), &[f64::INFINITY, 0.0]), "{got:?}");
    assert_eq!(ragged(&items, &[8]).map(|b| positions(&b)), Ok(vec![1, 5]));
}

#[test]
fn greedy_reports_loose_lines_as_they_are() {
    // No tolerance: a loose line is fine, and its ratio says how loose.
    //   0 B1, 1 G, 2 B1, 3 G, 4 B30, 5 forced
    let items = [bx(1), glue(1, 1, 0), bx(1), glue(1, 1, 0), bx(30), forced()];
    let got = greedy(&items, &[13]).unwrap();
    assert_eq!(positions(&got), [3, 5]);
    assert!(same_bits(&ratios(&got), &[10.0, f64::NEG_INFINITY]), "{got:?}");
    // With nothing to stretch, the ratio is infinite.
    let got = greedy(&[bx(1), glue(1, 1, 0), bx(30), forced()], &[10]).unwrap();
    assert!(same_bits(&ratios(&got), &[f64::INFINITY, f64::NEG_INFINITY]), "{got:?}");
}

#[test]
fn greedy_never_takes_more_lines_than_ragged() {
    // Whatever fits at its natural width fits when it can shrink, so greedy
    // stays level with ragged or ahead, line after line.
    let mut rng = Rng::new(0x9_4ead);
    let mut fewer = 0;
    for _ in 0..500 {
        let shape = Shape { words: rng.int(5, 60) as usize, forced: 0.05, fills: rng.chance(0.5), ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        let widths = random_widths(&mut rng, false, false, 6, 40);
        let (g, r) = (greedy(&items, &widths).unwrap(), ragged(&items, &widths).unwrap());
        assert!(g.len() <= r.len(), "greedy {g:?}\nragged {r:?}\n{}", repro_layout(&items, &widths));
        fewer += usize::from(g.len() < r.len());
    }
    assert!(fewer >= 100, "greedy only saved a line {fewer} times");
}

#[test]
fn without_shrink_greedy_and_ragged_agree() {
    let mut rng = Rng::new(0x5_4e1);
    for _ in 0..500 {
        let shape = Shape {
            words: rng.int(1, 50) as usize,
            forced: 0.1,
            fills: rng.chance(0.5),
            kerns: 0.3,
            penalties: 0.3,
            dyadic: rng.chance(0.5),
            ..Shape::default()
        };
        let items: Vec<Item> = paragraph(&mut rng, shape)
            .into_iter()
            .map(|it| match it {
                Item::Glue { width, stretch, .. } => Item::Glue { width, stretch, shrink: 0 },
                other => other,
            })
            .collect();
        let widths = random_widths(&mut rng, true, false, 1, 40);
        assert_eq!(
            greedy(&items, &widths).map(|b| positions(&b)),
            ragged(&items, &widths).map(|b| positions(&b)),
            "{}",
            repro_layout(&items, &widths)
        );
    }
}

// --- ragged

#[test]
fn ragged_never_stretches_or_shrinks() {
    let items = text(&hyphenate(CONFESSIONS));
    for width in 1..=80 {
        let got = ragged(&items, &[width * CH]).unwrap();
        assert!(got.iter().all(|b| b.ratio.to_bits() == 0), "at {width}: {got:?}");
    }
}

// --- against the oracles

/// Plain word wrap on strings, as an editor does it, with nothing from the
/// item model: `fits(letters, gaps)` says whether a line fits.
fn wrap<'a>(words: &[&'a str], fits: impl Fn(i64, i64) -> bool) -> Vec<Vec<&'a str>> {
    let mut lines = Vec::new();
    let mut line: Vec<&str> = Vec::new();
    for &word in words {
        let letters: i64 = line.iter().chain([&word]).map(|w| w.len() as i64).sum();
        if !line.is_empty() && !fits(letters, line.len() as i64) {
            lines.push(std::mem::take(&mut line));
        }
        line.push(word);
    }
    lines.push(line);
    lines
}

/// The words on each line, read off the breaks.
fn lines_of<'a>(words: &[&'a str], items: &[Item], breaks: &[Break]) -> Vec<Vec<&'a str>> {
    let mut words = words.iter();
    let mut start = 0;
    let mut lines = Vec::new();
    for b in breaks {
        let boxes = items[start..=b.position].iter().filter(|it| matches!(it, Item::Box { .. })).count();
        lines.push(words.by_ref().take(boxes).copied().collect());
        start = b.position + 1;
    }
    lines
}

#[test]
fn they_wrap_like_a_text_editor() {
    let mut rng = Rng::new(0xed17);
    let letters: Vec<char> = ('a'..='z').collect();
    for _ in 0..2000 {
        let words: Vec<String> = (0..rng.int(1, 40))
            .map(|_| (0..rng.int(1, 12)).map(|_| rng.pick(&letters)).collect())
            .collect();
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        let items = text(&words.join(" "));
        let width = rng.int(1, 40);
        // A space is one character; greedy may shrink it to three quarters.
        let ragged_wrap = wrap(&words, |letters, gaps| letters + gaps <= width);
        let greedy_wrap = wrap(&words, |letters, gaps| 4 * (letters + gaps) - gaps <= 4 * width);
        for (fit, want) in [(Fit::Ragged, ragged_wrap), (Fit::Greedy, greedy_wrap)] {
            let got = fit.run(&items, &[width * CH]).unwrap();
            assert_eq!(lines_of(&words, &items, &got), want, "{fit:?} at {width}");
        }
    }
}

/// How often the random trials hit the cases that matter, so they can't
/// quietly stop testing them.
#[derive(Debug, Default)]
struct Seen {
    lines: usize,
    too_long: usize,
    exact: usize,
    forced_waits: usize,
    comes_back: usize,
}

impl Seen {
    fn tally(&mut self, items: &[Item], widths: &[i64], fit: Fit, got: &[Break]) {
        let mut previous = None;
        for (k, b) in got.iter().enumerate() {
            let (s, e) = (line_start(items, previous), b.position);
            let w = line_width(widths, k);
            let m = measure(items, s, e);
            self.lines += 1;
            self.too_long += usize::from(!fit.fits(m, w));
            self.exact += usize::from(m.natural == w || (fit == Fit::Greedy && ratio(m, w) == -1.0));
            if fit.fits(m, w) && !is_forced(&items[e]) {
                let later: Vec<usize> = (e + 1..items.len()).filter(|&c| is_legal_break(items, c)).collect();
                self.forced_waits += usize::from(is_forced(&items[later[0]]));
                // Too long at the next break, but would it fit again later?
                let upto = later.iter().position(|&c| is_forced(&items[c])).unwrap();
                let fits_later = upto > 0 && later[1..=upto].iter().any(|&c| fit.fits(measure(items, s, c), w));
                self.comes_back += usize::from(fits_later);
            }
            previous = Some(e);
        }
    }
}

#[test]
fn random_paragraphs_match_first_fit() {
    for fit in Fit::BOTH {
        let mut rng = Rng::new(0xf1_25f1);
        let mut seen = Seen::default();
        for case in 0..3000 {
            let dyadic = case % 3 == 0;
            let shape = Shape {
                words: rng.int(1, 40) as usize,
                hyphens: 0.3,
                penalties: 0.2,
                forced: if case % 4 == 0 { 0.15 } else { 0.0 },
                fills: case % 2 == 0,
                kerns: if case % 5 == 0 { 0.4 } else { 0.0 },
                dyadic,
            };
            let items = paragraph(&mut rng, shape);
            let (lo, hi) = rng.pick(&[(1, 6), (4, 15), (10, 40)]);
            let widths = random_widths(&mut rng, case % 4 == 1, dyadic, lo, hi);
            let got = check_first_fit(&items, &widths, fit);
            assert_eq!(fit.run(&items, &widths), Ok(got.clone()), "{fit:?}: not the same answer twice");
            seen.tally(&items, &widths, fit, &got);
        }
        // A check that this test means something.
        assert!(seen.too_long >= 9000 && seen.exact >= 800 && seen.forced_waits >= 800, "{fit:?}: {seen:?}");
    }
}

/// Any old items: negative widths, negative stretch and shrink, infinite
/// glue anywhere, penalties of every cost and width, nothing in the usual
/// order.
fn soup(rng: &mut Rng) -> Vec<Item> {
    let costs = [i64::MIN, -INFINITY - 1, -INFINITY, -INFINITY + 1, -50, 0, 50, INFINITY - 1, INFINITY, i64::MAX];
    let mut items: Vec<Item> = (0..rng.int(0, 25))
        .map(|_| {
            let width = rng.pick(&[0, 1, 2, 3, 5, 8, -2]);
            match rng.int(0, 9) {
                0..=3 => bx(width),
                4 => infinite_glue(width, rng.int(-1, 3)),
                5 | 6 => glue(width, rng.int(-1, 4), rng.int(-1, 3)),
                _ => penalty(rng.pick(&[0, 0, 1, 2, -1]), rng.pick(&costs), rng.chance(0.5)),
            }
        })
        .collect();
    items.push(penalty(rng.pick(&[0, 0, 1, -1]), rng.pick(&costs[..3]), rng.chance(0.5)));
    items
}

#[test]
fn soup_matches_first_fit() {
    for fit in Fit::BOTH {
        let mut rng = Rng::new(0x50_0b);
        let mut seen = Seen::default();
        for _ in 0..20_000 {
            let items = soup(&mut rng);
            let widths: Vec<i64> = (0..rng.int(1, 3)).map(|_| rng.int(-3, 20)).collect();
            let got = check_first_fit(&items, &widths, fit);
            seen.tally(&items, &widths, fit, &got);
        }
        assert!(
            seen.too_long >= 10_000 && seen.exact >= 2000 && seen.forced_waits >= 1800 && seen.comes_back >= 200,
            "{fit:?}: {seen:?}"
        );
    }
}

#[test]
fn a_wider_column_never_needs_more_lines() {
    // Without negative widths, a line that fits also fits in anything wider,
    // and starting later only makes it shorter, so each line ends at least
    // as late as before.
    let mut rng = Rng::new(0x31de);
    for _ in 0..200 {
        let shape = Shape { words: rng.int(5, 50) as usize, forced: 0.05, fills: rng.chance(0.5), dyadic: rng.chance(0.5), ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        for fit in Fit::BOTH {
            let mut previous = usize::MAX;
            for width in (0..=60 * CH).step_by(250) {
                let lines = fit.run(&items, &[width]).unwrap().len();
                assert!(lines <= previous, "{fit:?}: {lines} lines at {width}, more than just narrower\n{}", repro_layout(&items, &[width]));
                previous = lines;
            }
        }
    }
}

// --- big paragraphs

fn words(n: usize) -> Arc<Vec<Item>> {
    Arc::new(text(&vec!["word"; n].join(" ")))
}

#[test]
fn a_million_words() {
    // Twelve words to a 60-character line, a million times over.
    let items = words(1_000_000);
    for fit in Fit::BOTH {
        let items = Arc::clone(&items);
        let got = within(Duration::from_secs(3), MIB / 4, "a million words", move || fit.run(&items, &[60 * CH])).unwrap();
        assert_eq!(got.len(), 83_334, "{fit:?}");
    }
}

#[test]
fn one_enormous_line() {
    // However long the line, each break is looked at once.
    let items = words(300_000);
    for fit in Fit::BOTH {
        let items = Arc::clone(&items);
        let got = within(Duration::from_secs(3), MIB / 4, "one enormous line", move || fit.run(&items, &[i64::MAX])).unwrap();
        assert_eq!(got.len(), 1, "{fit:?}");
    }
}

#[test]
fn a_line_for_every_word() {
    let items = words(1_000_000);
    for fit in Fit::BOTH {
        let items = Arc::clone(&items);
        let got = within(Duration::from_secs(3), MIB / 4, "a line for every word", move || fit.run(&items, &[0])).unwrap();
        assert_eq!(got.len(), 1_000_000, "{fit:?}");
    }
}
