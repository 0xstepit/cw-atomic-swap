# Run tests
test:
  @echo '🔬 Running tests...'
  cargo test --locked
  @echo '✅ Tests completed.'

# Run clippy
clippy:
  @echo '🔍 Running clippy...'
  cargo clippy --all-targets -- -D warnings
  @echo '✅ Clippy completed.'

# Run fmt
fmt:
  @echo '📐 Running fmt...'
  cargo fmt --all -- --check
  @echo '✅ Fmt completed.'

# Run codecoverage
tarpaulin:
  @echo 'z Running tarpaulin...'
  cargo tarpaulin
  @echo '✅ Tarpaulin completed.'

# Run checks required by github repo.
default-flow: fmt clippy test

# Run workspace optimizer
platform := if arch() =~ "aarch64" {"linux/arm64"} else {"linux/amd64"}
image := if arch() =~ "aarch64" {"cosmwasm/rust-optimizer-arm64:0.15.1"} else {"cosmwasm/rust-optimizer:0.15.1"}
optimize:
  @echo '🚀 Running build optimizer...'
  docker run --rm -v "$(pwd)":/code \
    --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
    --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
    --platform {{platform}} \
    {{image}}
  @echo '✅ Optimized build completed.'

schema:
  ./scripts/build_schema.sh
