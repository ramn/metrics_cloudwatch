#!/bin/bash
set -ex
# Usage: ./release.sh patch/minor/major VERSION

cargo install clog-cli && cargo install cargo-release

LEVEL=$1
VERSION=$2
if [ -z "$LEVEL" ]; then
    echo "Expected patch, minor or major"
    exit 1
fi

clog --$LEVEL
if [[ -z $(head -1 CHANGELOG.md | grep $VERSION) ]]; then
    git checkout CHANGELOG.md
    echo "Wrong version specified"
    exit 1
fi

git add CHANGELOG.md
git commit -m "Update changelog for $VERSION"

cargo release $LEVEL

git push upstream master --tags || true
