"""The oracle has to be right before it can judge anyone."""

import unittest
from dataclasses import replace

from trials import *


class TheJudges(unittest.TestCase):
    def test_the_oracle_agrees_with_brute_force(self):
        rng = Rng(0x0DD5_EED5)
        solved = 0
        for case in range(1500):
            dyadic = case % 3 == 0
            shape = Shape(
                words=rng.int(2, 9),
                hyphens=0.3,
                penalties=0.2,
                forced=0.15 if case % 5 == 0 else 0.0,
                fills=case % 2 == 0,
                kerns=0.3 if case % 7 == 0 else 0.0,
                dyadic=dyadic,
            )
            items = paragraph(rng, shape)
            if sum(1 for i in range(len(items)) if is_legal_break(items, i) and not is_forced(items[i])) > 16:
                continue
            widths = random_widths(rng, case % 4 == 0, dyadic, 6, 20)
            params = random_params(rng)
            if case % 6 == 0:
                params = replace(params, looseness=rng.int(-2, 2))
            a = oracle(items, widths, params)
            b = brute_force(items, widths, params)
            if a is None or b is None:
                self.assertEqual(a, b, repro(items, widths, params))
                continue
            self.assertEqual(a.ambiguous, b.ambiguous, repro(items, widths, params))
            if not a.ambiguous:
                self.assertEqual(a.lines, b.lines, repro(items, widths, params))
                self.assertTrue(close(a.demerits, b.demerits), f"{a.demerits} vs {b.demerits}\n{repro(items, widths, params)}")
                solved += 1
        self.assertGreater(solved, 500, f"only {solved} cases had a clear answer")

    def test_text_builds_what_it_says(self):
        self.assertEqual(
            text("ab c-de /"),
            [bx(2 * CH), SPACE, bx(CH), HYPHEN, bx(2 * CH), forbidden(), fill(), forced()],
        )
        self.assertEqual(hyphenate("restless creation"), "res-tle-ss cre-ati-on")

    def test_generated_paragraphs_keep_the_optimality_promise(self):
        rng = Rng(7)
        for _ in range(200):
            shape = Shape(words=40, kerns=0.3, forced=0.05, fills=True)
            self.assertTrue(right_monotone(paragraph(rng, shape)))

    def test_the_random_numbers_match_rust(self):
        # SplitMix64 from seed 0: the first outputs are well known.
        rng = Rng(0)
        self.assertEqual(rng.next_u64(), 0xE220_A839_7B1D_CDAF)
        self.assertEqual(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4)


if __name__ == "__main__":
    unittest.main()
