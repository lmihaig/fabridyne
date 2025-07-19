#!/usr/bin/env python3

import os
import subprocess
import json
import argparse
import sys
from typing import List, Any

# --- Configuration ---
TEST_DIR = "given_tests"
BUILD_COMMAND = ["cargo", "build", "--release"]
SIMULATOR_EXECUTABLE = os.path.join("target", "release", "ooo470")

INPUT_FILENAME = "input.json"
REFERENCE_OUTPUT_FILENAME = "output.json"
USER_OUTPUT_FILENAME = "user_output.json"


def deep_diff(expected: Any, actual: Any, path: str = "") -> List[str]:
    """
    Recursively compares two JSON-like structures and returns a list of
    strings describing the differences.
    """
    diffs = []

    if type(expected) != type(actual):
        diffs.append(
            f"Type mismatch at '{path}': Expected {type(expected).__name__}, got {type(actual).__name__}"
        )
        return diffs

    if isinstance(expected, dict):
        expected_keys, actual_keys = set(expected.keys()), set(actual.keys())
        if expected_keys != actual_keys:
            added = sorted(list(actual_keys - expected_keys))
            removed = sorted(list(expected_keys - actual_keys))
            if added:
                diffs.append(f"Keys added at '{path}': {added}")
            if removed:
                diffs.append(f"Keys removed at '{path}': {removed}")

        for key in sorted(list(expected_keys & actual_keys)):
            new_path = f"{path}.{key}" if path else key
            diffs.extend(deep_diff(expected.get(key), actual.get(key), path=new_path))

    elif isinstance(expected, list):
        if len(expected) != len(actual):
            diffs.append(
                f"List length mismatch at '{path}': Expected {len(expected)}, got {len(actual)}"
            )

        for i, (item_expected, item_actual) in enumerate(zip(expected, actual)):
            diffs.extend(deep_diff(item_expected, item_actual, path=f"{path}[{i}]"))

    elif expected != actual:
        diffs.append(
            f"Value mismatch at '{path}':\n  - Expected: {expected}\n  - Got:      {actual}"
        )

    return diffs


def compare_outputs(expected_file: str, actual_file: str) -> List[str]:
    """
    Compares two JSON files and returns a list of differences, highlighting the first cycle with an error.
    """
    try:
        with open(expected_file, "r") as f:
            expected_data = json.load(f)
        with open(actual_file, "r") as f:
            actual_data = json.load(f)

        # Pinpoint the exact cycle of the first difference
        for i, (expected_cycle, actual_cycle) in enumerate(
            zip(expected_data, actual_data)
        ):
            cycle_diffs = deep_diff(expected_cycle, actual_cycle, path="")
            if cycle_diffs:
                first_diff_summary = [f"First difference found in Cycle {i}:"]
                return first_diff_summary + cycle_diffs

        # If no differences in common cycles, check for length difference
        if len(expected_data) != len(actual_data):
            return [
                f"❌ Output has wrong number of cycles. Expected {len(expected_data)}, but got {len(actual_data)}."
            ]

        return []  # No differences found

    except json.JSONDecodeError as e:
        return [
            f"Error decoding JSON in '{e.doc.name}': {e.msg} at line {e.lineno} column {e.colno}"
        ]
    except FileNotFoundError as e:
        return [f"File not found: {e}"]


def run_single_test(test_dir: str, debug: bool) -> bool:
    """
    Runs a single test case located in its own directory.

    Args:
        test_dir: The path to the test directory (e.g., "given_tests/01-simple-add").

    Returns:
        True if the test passed, False otherwise.
    """
    test_name = os.path.basename(test_dir)
    print(f"--- Running Test: {test_name} ---")

    input_json = os.path.join(test_dir, INPUT_FILENAME)
    expected_output_json = os.path.join(test_dir, REFERENCE_OUTPUT_FILENAME)
    user_output_json = os.path.join(test_dir, USER_OUTPUT_FILENAME)

    # Check for required files
    if not os.path.exists(input_json):
        print(f"❌ Error: Input file not found: {input_json}")
        return False
    if not os.path.exists(expected_output_json):
        print(
            f"⚠️ Warning: No reference output file found at '{expected_output_json}'. Skipping comparison."
        )

    # Run the simulator
    executable = SIMULATOR_EXECUTABLE
    if debug:
        executable = executable.replace("release", "debug")
    run_command = [executable, input_json, user_output_json]
    try:
        subprocess.run(
            run_command, check=True, capture_output=True, text=True, timeout=10
        )
    except subprocess.TimeoutExpired:
        print(f"❌ Test Failed: Simulator timed out after 10 seconds.")
        return False
    except subprocess.CalledProcessError as e:
        print(f"❌ Test Failed: Simulator exited with error code {e.returncode}.")
        print(f"   Stdout: {e.stdout.strip()}")
        print(f"   Stderr: {e.stderr.strip()}")
        return False

    # Compare the results
    differences = compare_outputs(expected_output_json, user_output_json)

    if not differences:
        print("✅ Test Passed!")
        return True
    else:
        print("❌ Test Failed! Differences found:")
        for diff in differences:
            print("  " + diff.replace("\n", "\n  "))
        return False


def main():
    parser = argparse.ArgumentParser(description="Build and run simulator tests.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    # --- Build Command ---
    build_parser = subparsers.add_parser("build", help="Build the simulator.")
    build_parser.add_argument("--debug", action="store_true", help="Build in debug mode.")

    # --- Run Command ---
    run_parser = subparsers.add_parser("run", help="Run the simulator.")
    run_parser.add_argument("input", help="The input JSON file.")
    run_parser.add_argument("output", help="The output JSON file.")
    run_parser.add_argument("--debug", action="store_true", help="Run the debug build.")

    # --- Test Command ---
    test_parser = subparsers.add_parser("test", help="Run tests.")
    test_parser.add_argument(
        "test_number",
        nargs="?",
        type=int,
        help="The number of a specific test to run (e.g., '1' for the first test found). If not provided, all tests run.",
    )
    test_parser.add_argument("--debug", action="store_true", help="Run tests with the debug build.")

    args = parser.parse_args()

    if args.command == "build":
        print("--- Building Simulator ---")
        build_command = BUILD_COMMAND
        if args.debug:
            build_command = ["cargo", "build"]
        try:
            # Using --quiet to keep the build output clean
            subprocess.run(build_command, check=True, capture_output=True, text=True)
            print("✅ Build successful!")
        except subprocess.CalledProcessError as e:
            print(f"❌ Build Failed!\n   Error: {e.stderr.strip()}", file=sys.stderr)
            sys.exit(1)

    elif args.command == "run":
        executable = SIMULATOR_EXECUTABLE
        if args.debug:
            executable = executable.replace("release", "debug")
        run_command = [executable, args.input, args.output]
        try:
            subprocess.run(
                run_command, check=True, capture_output=True, text=True, timeout=10
            )
        except subprocess.TimeoutExpired:
            print(f"❌ Simulator timed out after 10 seconds.")
            sys.exit(1)
        except subprocess.CalledProcessError as e:
            print(f"❌ Simulator exited with error code {e.returncode}.")
            print(f"   Stdout: {e.stdout.strip()}")
            print(f"   Stderr: {e.stderr.strip()}")
            sys.exit(1)

    elif args.command == "test":
        try:
            # Discover tests by finding subdirectories in TEST_DIR
            test_dirs = sorted(
                [
                    os.path.join(TEST_DIR, d)
                    for d in os.listdir(TEST_DIR)
                    if os.path.isdir(os.path.join(TEST_DIR, d))
                ]
            )
            if not test_dirs:
                print(f"⚠️ No test directories found in '{TEST_DIR}'.", file=sys.stderr)
                sys.exit(1)
        except FileNotFoundError:
            print(f"❌ Error: Test directory '{TEST_DIR}' not found.", file=sys.stderr)
            sys.exit(1)

        if args.test_number is not None:
            if not (1 <= args.test_number <= len(test_dirs)):
                print(
                    f"❌ Error: Test number {args.test_number} is out of range.",
                    file=sys.stderr,
                )
                print(f"   Available tests: 1 to {len(test_dirs)}", file=sys.stderr)
                sys.exit(1)
            run_single_test(test_dirs[args.test_number - 1], args.debug)
        else:
            print("\n--- Running All Tests ---")
            passed_count = 0
            failed_tests = []
            for test_dir in test_dirs:
                if run_single_test(test_dir, args.debug):
                    passed_count += 1
                else:
                    failed_tests.append(os.path.basename(test_dir))

            print("\n--- Summary ---")
            print(f"Passed: {passed_count}/{len(test_dirs)}")
            if failed_tests:
                print("Failed tests:")
                for test in failed_tests:
                    print(f"  - {test}")


if __name__ == "__main__":
    main()
