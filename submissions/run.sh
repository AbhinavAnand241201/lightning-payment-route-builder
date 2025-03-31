#!/bin/bash

apt-get update && apt-get -y install cmake

# Check if three arguments are provided
if [ "$#" -ne 4 ]; then
    echo "Usage: $0 <output_file_path> <input_file_path> <payment_request_hex> <current_block_height>"
    exit 1
fi

# Assign arguments to variables
output_file_path="$1"
input_file_path="$2"
payment_request_hex="$3"
current_block_height="$4"

# Please fill in the version of the programming language you used here to help us with debugging if we run into problems!
version=20

# Check if the 'version' variable is not null
if [ -z $version ]; then
    echo "Please fill in the version of the programming language you used."
    exit 1
fi

# Build and run the Rust program
cargo build --manifest-path ./submissions/rust/Cargo.toml --release

./submissions/rust/target/release/route_builder "$output_file_path" "$input_file_path" "$payment_request_hex" "$current_block_height"