Releasing
=========

Releases are made from the [`release` GitHub Actions
workflow](https://github.com/ramn/metrics_cloudwatch/actions/workflows/release.yaml).
Open it, click "Run workflow" on `master` and enter the version to release:
`patch`, `minor`, `major` or an explicit version such as `5.1.0`.

The workflow uses [`cargo-release`](https://github.com/crate-ci/cargo-release)
(configured in `release.toml`) to

1. bump the version in `Cargo.toml`,
2. prepend the new entries to `CHANGELOG.md` using
   [`git-cliff`](https://git-cliff.org) (configured in `cliff.toml`),
3. commit as `chore: Version <version>`, publish to crates.io, tag
   `v<version>` and push to `master`,

and finally creates a GitHub release with the changelog for that version.

The changelog is generated from [conventional
commit](https://www.conventionalcommits.org) messages: `feat:`, `fix:`,
`perf:`, `doc:` and `revert:` commits are listed, other commits are left out.
Write commit messages (or squash-merge PR titles) in that form for them to show
up in the release notes.

To preview a release locally (requires `cargo-release` and `git-cliff`):

```bash
cargo release patch          # dry run, changes nothing
git cliff --unreleased --tag v5.0.1   # preview the changelog entry
```

Repository setup
----------------

The workflow runs in the `prod` environment, which needs these secrets:

* `CARGO_REGISTRY_TOKEN` - a crates.io API token with publish rights for the crate.
* `RELEASE_DEPLOY_KEY` - the private half of an SSH deploy key with write
  access, allowed to bypass the branch protection on `master`, so the release
  commit and tag can be pushed.
