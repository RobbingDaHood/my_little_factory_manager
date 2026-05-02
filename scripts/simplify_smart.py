#!/usr/bin/env python3
"""
Simplified strategy simplification loop using the diagnostic test for fast iteration.
Once a simplification is identified as good, confirm with the full test.
"""

import subprocess
import json
import re
from typing import Optional, Tuple

class StrategySimplifier:
    def __init__(self, use_diagnostic=True):
        self.use_diagnostic = use_diagnostic
        self.baseline_tier: Optional[int] = None
        self.baseline_actions: Optional[float] = None

    def run_test(self, test_name: str = "diagnostic", verbose: bool = True) -> Tuple[bool, int, float]:
        """
        Run either diagnostic or full test.
        Returns (success, max_tier_reached, mean_actions_to_tier_50)
        """
        if verbose:
            print(f"\n{'='*60}")
            print(f"Running {test_name} test...")
            print(f"{'='*60}")

        try:
            if self.use_diagnostic and "diagnostic" in test_name:
                cmd = [
                    "cargo", "test",
                    "--features", "simulation",
                    "--test", "simulation",
                    "--",
                    "--include-ignored",
                    "smart_strategy_diagnostic",
                    "--nocapture"
                ]
                timeout = 600  # 10 minutes
            else:
                cmd = [
                    "cargo", "test",
                    "--features", "simulation",
                    "--test", "simulation",
                    "--",
                    "--include-ignored",
                    "smart_strategy_reaches_tier_50",
                    "--nocapture"
                ]
                timeout = 1200  # 20 minutes

            result = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
            output = result.stderr + result.stdout

            # Parse the output
            max_tier = None
            mean_actions = None

            # Look for tier information
            lines = output.split('\n')
            for line in lines:
                if "overall_max_tier" in line:
                    match = re.search(r'"overall_max_tier":\s*(\d+)', line)
                    if match:
                        max_tier = int(match.group(1))

                # Parse milestone data
                if "Tier 50" in line and "reach" in line:
                    match = re.search(r'mean actions:\s*([0-9.]+|None)', line)
                    if match and match.group(1) != 'None':
                        mean_actions = float(match.group(1))

            success = result.returncode == 0 and max_tier is not None and max_tier >= 50

            if verbose:
                status = "✓ PASS" if success else "✗ FAIL"
                print(f"{status}: Max tier {max_tier}, Actions {mean_actions or 'N/A'}")
                if not success and max_tier:
                    print(f"   (Reached tier {max_tier}, need 50)")
                print(f"{'='*60}\n")

            return success, max_tier or 0, mean_actions or 0.0

        except subprocess.TimeoutExpired:
            if verbose:
                print(f"✗ TEST TIMEOUT\n")
            return False, 0, 0.0
        except Exception as e:
            if verbose:
                print(f"✗ TEST ERROR: {e}\n")
            return False, 0, 0.0

    def establish_baseline(self) -> bool:
        """Test the current strategy."""
        print("\n" + "="*60)
        print("PHASE 1: CHECKING CURRENT STRATEGY")
        print("="*60)

        success, tier, actions = self.run_test("diagnostic")

        if not success:
            print("ERROR: Current strategy doesn't reach tier 50!")
            print("Using full test to verify...")
            success, tier, actions = self.run_test("full", verbose=True)
            if not success:
                return False

        self.baseline_tier = tier
        self.baseline_actions = actions or 100000

        print(f"BASELINE: Tier {tier}, Actions {self.baseline_actions:.0f}")
        print(f"Target: Still reach tier 50, with time ≤ {self.baseline_actions * 1.2:.0f} actions")
        return True

    def try_change(self, name: str, make_change_fn, description: str) -> bool:
        """Try a change, test it, and revert if it fails."""
        print(f"\n{description}")

        # Make the change
        try:
            original = make_change_fn()
        except Exception as e:
            print(f"✗ Failed to apply change: {e}")
            return False

        # Test
        success, tier, actions = self.run_test("diagnostic", verbose=False)

        if success and tier >= 50 and (actions <= self.baseline_actions * 1.2 or actions <= 0):
            print(f"✓ Simplification accepted!")
            if actions > 0:
                print(f"  Tier: {tier}, Actions: {actions:.0f} ({actions/self.baseline_actions*100:.0f}% of baseline)")
            self.commit(description)
            return True
        else:
            print(f"✗ Simplification failed")
            if tier < 50:
                print(f"  Only reached tier {tier}")
            elif actions > self.baseline_actions * 1.2 and actions > 0:
                print(f"  Too slow: {actions:.0f} > {self.baseline_actions * 1.2:.0f}")
            self.revert(original)
            return False

    def read_file(self) -> str:
        """Read the strategy file."""
        with open("tests/simulation/strategies/smart_strategy.rs") as f:
            return f.read()

    def write_file(self, content: str):
        """Write the strategy file."""
        with open("tests/simulation/strategies/smart_strategy.rs", "w") as f:
            f.write(content)

    def commit(self, message: str):
        """Commit the change."""
        try:
            subprocess.run(
                ["git", "add", "tests/simulation/strategies/smart_strategy.rs"],
                check=True, capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", message],
                check=True, capture_output=True
            )
            print(f"  → Committed")
        except:
            print(f"  → Commit failed")

    def revert(self, original: str):
        """Revert to original content."""
        self.write_file(original)
        print(f"  → Reverted")

    def remove_worst_sacrifice_phases(self) -> str:
        """Simplify safe_sacrifice_index by removing phase 2 and 3."""
        content = self.read_file()
        original = content

        # Simplify the worst_sacrifice_for_tag function to remove refill/loop logic
        pattern = r'fn worst_sacrifice_for_tag\([^}]*\{[^}]*\n\s*loop \{[^}]*\n\s*\}[^}]*\n\s*\}'
        if re.search(pattern, content, re.DOTALL):
            # Just return None if we can't find the best sacrifice
            new_fn = '''    fn worst_sacrifice_for_tag(
        &self,
        tag: &CardTag,
        cards: &[CardEntry],
        replacement_indices_raw: &[usize],
        hash_to_index: &HashMap<u64, usize>,
        exclude: usize,
        sacrifice_indices: &[usize],
    ) -> Option<usize> {
        // Simplified: just find any card with this tag in sacrifice_indices
        sacrifice_indices
            .iter()
            .copied()
            .filter(|&i| i != exclude && cards[i].card.tags.iter().any(|t| t == tag))
            .min_by(|&a, &b| Self::by_quality(cards, a, b))
    }'''

            # Replace the function
            pattern = r'fn worst_sacrifice_for_tag\(.*?\n\s*\}'
            content = re.sub(pattern, new_fn, content, count=1, flags=re.DOTALL)

        if content != original:
            self.write_file(content)

        return original

    def reduce_tracking_constants(self) -> str:
        """Reduce best/worst tracking from 30 to 20."""
        content = self.read_file()
        original = content

        # Reduce TOP_N and BOTTOM_N
        content = re.sub(r'const TOP_N: usize = \d+', 'const TOP_N: usize = 20', content)
        content = re.sub(r'const BOTTOM_N: usize = \d+', 'const BOTTOM_N: usize = 20', content)

        if content != original:
            self.write_file(content)

        return original

    def reduce_pass1_candidates(self) -> str:
        """Reduce PASS1_CANDIDATES from 200 to 100."""
        content = self.read_file()
        original = content

        content = re.sub(r'const PASS1_CANDIDATES: usize = \d+', 'const PASS1_CANDIDATES: usize = 100', content)

        if content != original:
            self.write_file(content)

        return original

    def relax_slow_progress_limit(self) -> str:
        """Relax SLOW_PROGRESS_TURN_LIMIT from 60 to 80."""
        content = self.read_file()
        original = content

        content = re.sub(r'const SLOW_PROGRESS_TURN_LIMIT: u32 = \d+', 'const SLOW_PROGRESS_TURN_LIMIT: u32 = 80', content)

        if content != original:
            self.write_file(content)

        return original

    def run_simplification_loop(self):
        """Run the simplification loop."""
        print("\n" + "="*60)
        print("SMART STRATEGY SIMPLIFICATION")
        print("="*60)

        if not self.establish_baseline():
            return

        print("\n" + "="*60)
        print("PHASE 2: SYSTEMATIC SIMPLIFICATION")
        print("="*60)

        # List of simplifications to try
        simplifications = [
            ("reduce_tracking_constants", self.reduce_tracking_constants, "Reduce card tracking from 30→20"),
            ("reduce_pass1_candidates", self.reduce_pass1_candidates, "Reduce pass1 candidates from 200→100"),
            ("relax_slow_progress", self.relax_slow_progress_limit, "Relax slow progress limit from 60→80"),
            ("simplify_sacrifice", self.remove_worst_sacrifice_phases, "Simplify worst_sacrifice_for_tag logic"),
        ]

        successful = 0
        for name, fn, description in simplifications:
            if self.try_change(name, fn, description):
                successful += 1

        print("\n" + "="*60)
        print(f"SIMPLIFICATIONS COMPLETE: {successful}/{len(simplifications)} accepted")
        print("="*60)

        # Final verification with full test
        print("\nRunning FINAL VERIFICATION with full test...")
        success, tier, actions = self.run_test("full")
        if success and tier >= 50:
            print(f"✓ SUCCESS: All simplifications still reach tier {tier}!")
        else:
            print(f"✗ WARNING: Full test failed - some simplifications may be too aggressive")

if __name__ == "__main__":
    simplifier = StrategySimplifier(use_diagnostic=True)
    simplifier.run_simplification_loop()
