##
## General
##

# List available recipes
usage:
    @just --list --unsorted --list-prefix "  " --justfile "{{ justfile() }}"

##
## Build and run
##

# Generate measurements file
[group('build and run')]
generate:
    @cargo run --release --features generator --bin generate 1000000000

# Build release version
[group('build and run')]
build:
    @cargo build --release

# Run once with printing output and measure time
[group('build and run')]
run: build
    @time ./target/release/obrc

##
## Benchmarking
##

# Run benchmark with 10 runs and 2 warmup runs
[group('benchmarking')]
benchmark: build
    @hyperfine --warmup 2 --runs 10 ./target/release/obrc

# Run benchmark once with no warmup
[group('benchmarking')]
benchmark-once: build
    @hyperfine --runs 1 ./target/release/obrc

##
## Development
##

# Run with dtrace and generate flamegraph
[group('development')]
dtrace: build
    @sudo dtrace -c './target/release/obrc' -n 'profile-997 /execname == "obrc"/ { @[ustack(100)] = count(); } tick-10s { exit(0); }' -o out.user_stacks
    @cat out.user_stacks | inferno-collapse-dtrace | inferno-flamegraph > perf.svg
    @sudo rm out.user_stacks
    @open perf.svg

# Show assembly of function (e.g.: just asm obrc::main)
[group('development')]
asm function *index:
    @cargo asm --bin obrc --rust --this-workspace '{{ function }}' {{ index }}
