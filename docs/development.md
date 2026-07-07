# Development guide

Material moved out of the README, which is end-user facing.

## Build and test

```bash
cargo build --release        # binary at target/release/meld
cargo test                   # integration tests on synthetic temp trees
./scripts/smoke-test.sh      # end-to-end run in a sandboxed $HOME
```

Tests never touch the real `~/Library/Application Support/Claude` path.
See `CLAUDE.md` for invariants and gotchas (e.g. the 2-second write-quiet
guard means test fixtures must be backdated with `touch -t`).

## Publishing a release

1. Push the repo to `github.com/siddhjagani/meld`.
2. Tag: `git tag v0.1.0 && git push --tags`. The workflow in
   `.github/workflows/release.yml` builds and tests arm64 + x86_64 macOS
   binaries and attaches `meld-<target>.tar.gz` plus `.sha256` files to the
   GitHub release.
3. The curl installer (`install.sh`) then works with no further steps — it
   pulls the latest release asset for the user's architecture.
4. Homebrew: copy `Formula/meld.rb` into a `siddhjagani/homebrew-tap` repo
   and paste the two sha256 values from the release assets. Users install
   with `brew install siddhjagani/tap/meld`. (The name `meld` in
   homebrew-core belongs to the GUI diff tool, hence the tap.)

## Custom-domain installer

To offer `curl -fsSL https://yourdomain.com/install.sh | bash`, serve
`install.sh` from that URL (a 301 redirect to the raw GitHub URL is enough).
