set shell := ["bash", "-cu"]

# Override with: just VERSION=v0.2.0 release
VERSION := "v0.1.0"
HOMEPAGE := "https://github.com/furiosa-ai/kubectl-view-rngd"

# Default artifact URL prefix; the release workflow overrides this.
ARTIFACT_BASE := "https://github.com/furiosa-ai/kubectl-view-rngd/releases/download"

BIN_NAME := "kubectl-view-rngd"
PKG_PREFIX := "kubectl-view-rngd"

# Convenience aggregates.
default: check test

check:
    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings

test:
    cargo test --all-targets

build:
    cargo build --release

# Cross-compile the four release targets. Requires `cross` installed for linux
# targets, and a macOS host (or `cargo-zigbuild`) for darwin targets.
cross-all:
    cross build --release --target x86_64-unknown-linux-gnu
    cross build --release --target aarch64-unknown-linux-gnu
    cargo build --release --target x86_64-apple-darwin
    cargo build --release --target aarch64-apple-darwin

# Package a single target into a tarball with the binary + LICENSE + README.
# Usage: just package linux amd64 x86_64-unknown-linux-gnu
package os arch triple:
    mkdir -p dist/staging dist
    cp target/{{triple}}/release/{{BIN_NAME}} dist/staging/
    cp LICENSE README.md dist/staging/
    tar -C dist/staging -czf dist/{{PKG_PREFIX}}_{{VERSION}}_{{os}}_{{arch}}.tar.gz .
    rm -rf dist/staging

package-all: cross-all
    just package linux  amd64 x86_64-unknown-linux-gnu
    just package linux  arm64 aarch64-unknown-linux-gnu
    just package darwin amd64 x86_64-apple-darwin
    just package darwin arm64 aarch64-apple-darwin
    (cd dist && shasum -a 256 *.tar.gz > SHA256SUMS.txt)

# Render the krew manifest from the template + dist/SHA256SUMS.txt.
# Result is written to dist/view-rngd.yaml.
render-manifest:
    #!/usr/bin/env bash
    set -euo pipefail
    declare -A sha
    while read -r hash file; do sha["$file"]="$hash"; done < dist/SHA256SUMS.txt
    base="{{ARTIFACT_BASE}}/{{VERSION}}"
    export VERSION="{{VERSION}}"
    export HOMEPAGE="{{HOMEPAGE}}"
    export URI_LINUX_AMD64="$base/{{PKG_PREFIX}}_{{VERSION}}_linux_amd64.tar.gz"
    export URI_LINUX_ARM64="$base/{{PKG_PREFIX}}_{{VERSION}}_linux_arm64.tar.gz"
    export URI_DARWIN_AMD64="$base/{{PKG_PREFIX}}_{{VERSION}}_darwin_amd64.tar.gz"
    export URI_DARWIN_ARM64="$base/{{PKG_PREFIX}}_{{VERSION}}_darwin_arm64.tar.gz"
    export SHA_LINUX_AMD64="${sha[{{PKG_PREFIX}}_{{VERSION}}_linux_amd64.tar.gz]}"
    export SHA_LINUX_ARM64="${sha[{{PKG_PREFIX}}_{{VERSION}}_linux_arm64.tar.gz]}"
    export SHA_DARWIN_AMD64="${sha[{{PKG_PREFIX}}_{{VERSION}}_darwin_amd64.tar.gz]}"
    export SHA_DARWIN_ARM64="${sha[{{PKG_PREFIX}}_{{VERSION}}_darwin_arm64.tar.gz]}"
    envsubst < .krew/view-rngd.yaml.tmpl > dist/view-rngd.yaml
    echo "wrote dist/view-rngd.yaml"

# Full release: cross-compile, package, hash, render manifest. The pipeline
# caller is responsible for publishing dist/*.tar.gz and dist/view-rngd.yaml.
release: package-all render-manifest

# Verify the rendered manifest against a local tarball without uploading.
# Usage: just verify-manifest linux amd64
verify-manifest os arch:
    kubectl krew install --manifest=dist/view-rngd.yaml \
        --archive=dist/{{PKG_PREFIX}}_{{VERSION}}_{{os}}_{{arch}}.tar.gz

clean:
    cargo clean
    rm -rf dist
