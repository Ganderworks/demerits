"""Run the Python trials up to a level.

    python3 python/run.py            # the current level
    python3 python/run.py medium     # peek at a harder one

Each level includes the ones below it, and the judges' own checks always
run. Raise LEVEL when you pass.
"""

import sys
import unittest
from pathlib import Path

LEVEL = "easy"
LEVELS = ["easy", "medium"]

here = Path(__file__).resolve().parent
sys.path.insert(0, str(here))
level = sys.argv[1] if len(sys.argv) > 1 else LEVEL
if level not in LEVELS:
    sys.exit(f"no level {level!r}; the Python trials have {', '.join(LEVELS)}")
names = ["test_trials"] + [f"test_{name}" for name in LEVELS[: LEVELS.index(level) + 1]]
result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromNames(names))
sys.exit(0 if result.wasSuccessful() else 1)
