#!/bin/bash
set -e

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 </path/to/input.json> </path/to/output.json>"
    exit 1
fi

timeout 5s ./target/release/ooo470 "$1" "$2"
