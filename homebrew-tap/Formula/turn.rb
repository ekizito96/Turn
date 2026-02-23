# typed: false
# frozen_string_literal: true

class Turn < Formula
  desc "A systems language for agentic computation"
  homepage "https://github.com/ekizito96/Turn"
  version "0.5.0-alpha"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ekizito96/Turn/releases/download/v#{version}/turn-macos-arm64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_ARM64"
    end
    on_intel do
      url "https://github.com/ekizito96/Turn/releases/download/v#{version}/turn-macos-amd64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_AMD64"
    end
  end

  on_linux do
    url "https://github.com/ekizito96/Turn/releases/download/v#{version}/turn-linux-amd64.tar.gz"
    sha256 "PLACEHOLDER_SHA256_LINUX"
  end

  def install
    bin.install "turn"
  end

  test do
    assert_match "turn", shell_output("#{bin}/turn --version")
  end
end
