# List available recipes
usage:
    @just --list --unsorted --list-prefix "  " --justfile "{{justfile()}}"

# Generate measurements file
generate:
    @cargo run --release --features generator --bin generate 1000000000

# Build release version
build:
    @cargo build --release

# Run benchmark with 10 runs and 2 warmup runs
benchmark: build
    @hyperfine --warmup 2 --runs 10 ./target/release/obrc

# Run benchmark once
benchmark-once: build
    @hyperfine --runs 1 ./target/release/obrc

# Run once with printing output and measure time
run: build
    @time ./target/release/obrc
