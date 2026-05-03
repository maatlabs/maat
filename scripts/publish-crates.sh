#!/bin/sh

# Script to publish all Maat crates to `crates.io`.
# Usage: ./publish-crates.sh [args]
#
# e.g.:  ./publish-crates.sh
#        ./publish-crates.sh --dry-run

set -e

# Checkout
echo "Checking out `main` branch..."
git checkout main
git pull origin main

# Publish
echo "Publishing crates..."
crates=(
maat_span
maat_stdlib
maat_errors
maat_field
maat_lexer
maat_ast
maat_parser
maat_runtime
maat_types
maat_eval
maat_bytecode
maat_codegen
maat_vm
maat_trace
maat_air
maat_prover
maat_module
maat
)
for crate in ${crates[@]}; do
    echo "Publishing $crate..."
    cargo publish -p "$crate" $@
done
