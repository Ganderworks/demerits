"""Medium: is it really Knuth–Plass, or greedy in a nice coat?"""

import unittest
from dataclasses import replace

from trials import *


class Medium(unittest.TestCase):
    def test_ratios_are_what_the_glue_has_to_do(self):
        # [B16, G(4, +8, -2), B16]: natural 36, stretch 8, shrink 2.
        items = [bx(16), glue(4, 8, 2), bx(16), forced()]
        p = Params()
        self.assertEqual(break_lines(items, [44], p), [Break(3, 1.0)])
        self.assertEqual(break_lines(items, [35], p), [Break(3, -0.5)])
        self.assertEqual(break_lines(items, [36], p), [Break(3, 0.0)])

    def test_tolerance_decides_what_is_feasible(self):
        # One line, ratio (20 - 13) / 1 = 7. Breaking at the glue leaves a
        # lone box with no stretch, which is never feasible.
        items = [bx(6), glue(1, 1, 1), bx(6), forced()]
        with self.assertRaises(Infeasible):
            break_lines(items, [20], Params(tolerance=5.0))
        self.assertEqual(break_lines(items, [20], Params(tolerance=8.0)), [Break(3, 7.0)])

    def test_it_beats_greedy(self):
        items = text(CONFESSIONS)
        params = Params()
        beaten = []
        for width in range(20, 61):
            widths = [width * CH]
            if check(items, widths, params) != Verdict.SOLVED:
                continue
            ours = validate(items, widths, params, break_lines(items, widths, params)).demerits
            try:
                theirs = evaluate(items, widths, params, greedy(items, widths)).demerits
            except Invalid:
                theirs = None
            if theirs is None or not close(theirs, ours):
                beaten.append(width)
        # Not a check on you (check() already held you to the optimum); a
        # check that this test means something.
        self.assertGreaterEqual(len(beaten), 10, f"greedy only lost at widths {beaten}")

    def test_line_penalty_trades_lines_for_spacing(self):
        rng = Rng(0x11E)
        differ = 0
        for _ in range(200):
            items = paragraph(rng, Shape(words=40))
            widths = random_widths(rng, False, False, 18, 35)
            cheap = Params(line_penalty=0.0, tolerance=5.0)
            dear = replace(cheap, line_penalty=1000.0)
            if check(items, widths, cheap) != Verdict.SOLVED or check(items, widths, dear) != Verdict.SOLVED:
                continue
            a = len(break_lines(items, widths, cheap))
            b = len(break_lines(items, widths, dear))
            self.assertLessEqual(b, a, f"a dearer line made more lines ({b} > {a})\n{repro(items, widths, dear)}")
            if b < a:
                differ += 1
        self.assertGreaterEqual(differ, 2, f"line_penalty only mattered {differ} times")

    def test_penalties_steer_the_breaks(self):
        rng = Rng(0xBAD_C0DE)
        solved = 0
        for _ in range(400):
            shape = Shape(words=rng.int(3, 11), hyphens=0.0, penalties=0.5)
            items = paragraph(rng, shape)
            widths = random_widths(rng, False, False, 8, 20)
            if check_brute(items, widths, Params(tolerance=3.0)) == Verdict.SOLVED:
                solved += 1
        self.assertGreaterEqual(solved, 200, f"only {solved} of 400 had a clear answer")

    def test_small_paragraphs_match_brute_force(self):
        rng = Rng(42)
        solved = 0
        for _ in range(500):
            shape = Shape(words=rng.int(2, 12), hyphens=0.0, penalties=0.0)
            items = paragraph(rng, shape)
            widths = random_widths(rng, False, False, 8, 24)
            params = random_params(rng)
            if check_brute(items, widths, params) == Verdict.SOLVED:
                solved += 1
        self.assertGreaterEqual(solved, 250, f"only {solved} of 500 had a clear answer")


if __name__ == "__main__":
    unittest.main()
