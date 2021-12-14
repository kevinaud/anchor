#!/bin/bash

# Delete existing file outputs from macrotest
rm $(git rev-parse --show-toplevel)/lang/tests/expand/*.expanded.rs

# Regenerated expanded files
cargo test expand