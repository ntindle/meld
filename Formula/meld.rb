# Homebrew formula for meld. Lives in your tap repo (siddhjagani/homebrew-tap)
# as Formula/meld.rb, so users install with:
#   brew install siddhjagani/tap/meld
#
# After each release, update `url`/`sha256` for both architectures
# (the release workflow prints the sha256 files as assets).
class Meld < Formula
  desc "Merge Claude Code sessions across accounts - safely, locally, reversibly"
  homepage "https://github.com/siddhjagani/meld"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/siddhjagani/meld/releases/download/v#{version}/meld-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ARM64_SHA256"
    else
      url "https://github.com/siddhjagani/meld/releases/download/v#{version}/meld-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_SHA256"
    end
  end

  def install
    bin.install "meld"
  end

  test do
    assert_match "meld", shell_output("#{bin}/meld --version")
  end
end
