# CI Matrix Plan — openguild-server

## Goals
- Validate the backend server crate on both Linux (ubuntu-latest) and Windows (windows-latest).
- Execute formatting, linting, build, and tests (with/without metrics feature).
- Cache Cargo build artifacts to keep pipelines fast.
- Ensure feature-gated `/metrics` checks run at least once per schedule.

## GitHub Actions Outline

```yaml
name: backend-ci

on:
  push:
    branches: [ main ]
  pull_request:
  schedule:
    - cron: '0 6 * * *'  # daily sanity run

jobs:
  check:
    strategy:
      fail-fast: false
      matrix:
        os: [ ubuntu-latest, windows-latest ]
    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - uses: dtolnay/rust-toolchain@stable

      - name: Format
        run: make fmt

      - name: Lint
        run: make lint

      - name: Backend Check
        run: make check

      - name: Backend Tests
        run: make test

  metrics-feature:
    needs: check
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4
      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: metrics-${{ hashFiles('**/Cargo.lock') }}
      - uses: dtolnay/rust-toolchain@stable
      - name: Test with metrics feature
        run: make test-metrics

  security-audit:
    if: github.event_name == 'schedule'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-audit --locked
      - run: cargo audit
```

## Follow-ups
- Add macOS job if we adopt Apple tooling or dependencies.
- Incorporate frontend lint/test jobs once the Nuxt app matures.
- Gate merges on CI success using required checks.
