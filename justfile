# Justfile

fmt:
    cargo +nightly fmt --all

clippy:
    cargo +nightly clippy

clean:
    git clean -dfx

build:
    cargo build --profile release-lto

install:
    cargo install --profile release-lto --all-features --timings --path .

release:
    cargo smart-release --no-push --no-changelog-preview --allow-dirty --no-publish --execute
