#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?Usage: scripts/release.sh <version>}"

# Validate format: must be v1.2.3
if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Version must be in the form v1.2.3 (got: $VERSION)"
    exit 1
fi

BARE_VERSION="${VERSION#v}"

# Ensure we are at the project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

# Ensure the tag doesn't already exist
jj sync
if [[ -n "$(jj tag list "exact:$VERSION")" ]]; then
    echo "Error: Tag $VERSION already exists"
    exit 1
fi

echo "==> Preparing release $VERSION"

# Generate the changelog now that the tag is in place
echo "    Generating CHANGELOG.md..."
RUST_LOG='' git-cliff --config .config/cliff.toml -t "${VERSION}" >CHANGELOG.md
just fmt

# Update version in [workspace.package] section of Cargo.toml
echo "    Updating Cargo.toml..."
awk -v ver="$BARE_VERSION" '
  /^\[workspace\.package\]/ { in_section = 1 }
  /^\[/ && in_section && !/^\[workspace\.package\]/ { in_section = 0 }
  in_section && /^version = / && !done {
    sub(/version = "[^"]*"/, "version = \"" ver "\"")
    done = 1
  }
  { print }
' Cargo.toml >Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

# Refresh Cargo.lock to reflect the new workspace version
echo "    Updating Cargo.lock..."
cargo fetch --quiet

# Set the jj commit description before tagging
echo "    Setting commit description..."
jj desc -m "chore(release): Preparing release for version $VERSION"
jj new
jj tug

# Sign before tagging: tags() is in immutable_heads(), so once tagged the commit
# becomes immutable and git.sign-on-push will refuse to sign it.
echo "    Signing release commit..."
jj sign -r @-

# Tag the signed release commit
echo "    Tagging $VERSION..."
jj tag set -r @- "$VERSION"

# Push bookmark and tag to GitHub (triggers the release workflow).
# --tag auto-tracks the new remote tag; a plain push refuses untracked tags.
echo "    Pushing..."
jj git push --bookmark main --tag "$VERSION"
jj sync

echo "==> Release $VERSION pushed. Monitor the GitHub Actions workflow for progress."
