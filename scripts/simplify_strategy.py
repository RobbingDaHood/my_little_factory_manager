#!/usr/bin/env python3
"""
Systematic simplification loop for SmartStrategy.
Tests if the strategy still reaches tier 50 after each simplification.
Commits successful simplifications and rolls back failures.
"""

import subprocess
import json
import time
import sys
from typing import Optional, Tuple

class StrategySimplifier:
    def __init__(self):
        self.baseline_actions: Optional[float] = None
        self.baseline_tier: Optional[int] = None
        self.max_time_multiplier = 1.20  # 20% slower is acceptable

    def run_test(self, name: str = "test") -> Tuple[bool, int, float]:
        """
        Run the simulation test and return (success, max_tier, mean_actions).
        Returns (False, 0, 0.0) if test fails.
        """
        print(f"\n{'='*60}")
        print(f"Running test: {name}")
        print(f"{'='*60}")

        try:
            result = subprocess.run(
                [
                    "cargo", "test",
                    "--features", "simulation",
                    "--test", "simulation",
                    "--",
                    "--include-ignored",
                    "smart_strategy_reaches_tier_50",
                    "--nocapture"
                ],
                capture_output=True,
                text=True,
                timeout=900  # 15 minutes per test
            )

            output = result.stderr + result.stdout

            # Only print last 50 lines for brevity
            output_lines = output.split('\n')
            print('\n'.join(output_lines[-50:]))

            # Parse results from JSON and text output
            max_tier = None
            mean_actions = None

            # Look for the milestone output
            import re
            for line in output_lines:
                if "Milestone tier 50" in line:
                    match = re.search(r'mean actions:\s*([0-9.]+)', line)
                    if match:
                        mean_actions = float(match.group(1))

            # Try to parse the full JSON report if present
            try:
                for line in output_lines:
                    if '"overall_max_tier"' in line:
                        report = json.loads(line)
                        max_tier = report.get('overall_max_tier')
                        for milestone in report.get('milestones', []):
                            if milestone['tier'] == 50 and milestone.get('mean_actions'):
                                mean_actions = milestone.get('mean_actions')
                        break
            except:
                pass

            success = result.returncode == 0 and max_tier is not None and max_tier >= 50

            print(f"\n{'='*60}")
            if success:
                print(f"✓ TEST PASSED: Reached tier {max_tier}")
                if mean_actions:
                    print(f"  Mean actions to tier 50: {mean_actions:.0f}")
            else:
                print(f"✗ TEST FAILED: Max tier {max_tier}, return code {result.returncode}")
            print(f"{'='*60}\n")

            return success, max_tier or 0, mean_actions or 0.0

        except subprocess.TimeoutExpired:
            print(f"✗ TEST TIMEOUT")
            return False, 0, 0.0
        except Exception as e:
            print(f"✗ TEST ERROR: {e}")
            import traceback
            traceback.print_exc()
            return False, 0, 0.0

    def get_baseline(self) -> bool:
        """Establish baseline metrics (current state)."""
        print("\n" + "="*60)
        print("PHASE 1: ESTABLISHING BASELINE")
        print("="*60)

        success, tier, actions = self.run_test("baseline")

        if not success:
            print("ERROR: Current strategy doesn't reach tier 50!")
            return False

        self.baseline_tier = tier
        self.baseline_actions = actions
        print(f"\nBASELINE ESTABLISHED:")
        print(f"  Max tier: {tier}")
        print(f"  Mean actions to tier 50: {actions:.0f}")
        print(f"  Acceptable time: up to {actions * self.max_time_multiplier:.0f} actions")

        return True

    def git_commit(self, message: str) -> bool:
        """Commit changes with the given message."""
        try:
            subprocess.run(
                ["git", "add", "tests/simulation/strategies/smart_strategy.rs"],
                check=True,
                capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", message],
                check=True,
                capture_output=True
            )
            print(f"✓ Committed: {message}")
            return True
        except subprocess.CalledProcessError as e:
            print(f"✗ Commit failed: {e}")
            return False

    def git_diff(self) -> str:
        """Get the current diff."""
        result = subprocess.run(
            ["git", "diff", "tests/simulation/strategies/smart_strategy.rs"],
            capture_output=True,
            text=True
        )
        return result.stdout

    def git_checkout_file(self) -> bool:
        """Revert the file to the last commit."""
        try:
            subprocess.run(
                ["git", "checkout", "tests/simulation/strategies/smart_strategy.rs"],
                check=True,
                capture_output=True
            )
            print(f"✓ Reverted file")
            return True
        except subprocess.CalledProcessError:
            return False

    def simplify_constant(self, const_name: str, new_value: str) -> bool:
        """Try to simplify by changing a constant."""
        print(f"\nTrying: simplify constant {const_name} → {new_value}")

        # Read the file
        with open("tests/simulation/strategies/smart_strategy.rs", "r") as f:
            content = f.read()

        # Find and replace the constant
        import re
        pattern = rf'(const {const_name}:\s*\w+\s*=\s*)[^;]*;'
        match = re.search(pattern, content)
        if not match:
            print(f"✗ Could not find constant {const_name}")
            return False

        new_content = re.sub(pattern, rf'\1{new_value};', content, count=1)

        if new_content == content:
            print(f"✗ No change made for {const_name}")
            return False

        # Write back
        with open("tests/simulation/strategies/smart_strategy.rs", "w") as f:
            f.write(new_content)

        print(f"  Changed in code, testing...")

        # Test
        success, tier, actions = self.run_test(f"simplify {const_name}")

        if success and tier >= 50 and actions <= self.baseline_actions * self.max_time_multiplier:
            print(f"✓ SIMPLIFICATION ACCEPTED: {const_name} → {new_value}")
            print(f"  Tier: {tier}, Actions: {actions:.0f} (baseline: {self.baseline_actions:.0f})")
            self.git_commit(f"Simplify: reduce {const_name} to {new_value}")
            return True
        else:
            print(f"✗ SIMPLIFICATION REJECTED")
            if tier < 50:
                print(f"  Reason: only reached tier {tier}")
            elif actions > self.baseline_actions * self.max_time_multiplier:
                print(f"  Reason: too slow ({actions:.0f} > {self.baseline_actions * self.max_time_multiplier:.0f})")
            self.git_checkout_file()
            return False

    def run_simplification_loop(self):
        """Run the full simplification loop."""
        print("\n" + "="*60)
        print("SMART STRATEGY SIMPLIFICATION LOOP")
        print("="*60)

        # Establish baseline
        if not self.get_baseline():
            return

        print("\n" + "="*60)
        print("PHASE 2: SYSTEMATIC SIMPLIFICATION")
        print("="*60)
        print("\nTrying to simplify constants and parameters...")
        print("(Will test each change and commit if successful)\n")

        # List of simplifications to try, in order
        # Each tuple is (constant_name, new_value, description)
        simplifications = [
            ("PASS1_CANDIDATES", "100", "Reduce pass1 candidates (diversity forcing)"),
            ("TOP_N", "20", "Reduce top-N best card tracking"),
            ("BOTTOM_N", "20", "Reduce bottom-N worst card tracking"),
            ("NO_RESOLUTION_STUCK_THRESHOLD", "1000", "Tighten livelock detection"),
            ("SLOW_PROGRESS_TURN_LIMIT", "50", "Stricter slow progress detection"),
            ("DISCARD_STUCK_THRESHOLD", "40", "Earlier discard stuck detection"),
        ]

        successful = 0
        total = 0

        for const_name, new_value, description in simplifications:
            total += 1
            print(f"\n[{total}/{len(simplifications)}] {description}")
            if self.simplify_constant(const_name, new_value):
                successful += 1

        print("\n" + "="*60)
        print(f"SIMPLIFICATION COMPLETE: {successful}/{total} simplifications accepted")
        print("="*60)
        print("\nRun final test to confirm everything still works...")

        success, tier, actions = self.run_test("final verification")
        if success and tier >= 50:
            print(f"\n✓ FINAL VERIFICATION PASSED: Tier {tier}, Actions {actions:.0f}")
        else:
            print(f"\n✗ FINAL VERIFICATION FAILED")

if __name__ == "__main__":
    simplifier = StrategySimplifier()
    simplifier.run_simplification_loop()
