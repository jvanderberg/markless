---
description: Cut a release - run checks, bump version, merge PR, tag, and publish to crates.io
user_invocable: true
---

# Release

Cut a new release of markless. Follow these steps in order:

## 1. Run full checks

Run `./scripts/check.sh` to ensure formatting, clippy, and all tests pass. If anything fails, fix it before proceeding.

## 2. Increment the version

Bump the `version` field in `Cargo.toml` by `0.0.1` (patch increment) unless the user specifies a different version or increment. For example, `0.9.6` becomes `0.9.7`.

After editing `Cargo.toml`, run `cargo check` so that `Cargo.lock` is updated to match.

## 3. Commit and push

Stage `Cargo.toml` and `Cargo.lock`, then commit with the message `Release vX.Y.Z` (using the new version number). Push to the current branch.

## 4. Merge the PR

Use `gh pr merge --squash` to merge the current branch's PR into main. If there is no open PR for the current branch, skip this step and inform the user.

## 5. Tag the release

After merging, check out `main`, pull, then create and push an annotated tag:

```
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

## 6. Publish to crates.io

Run `cargo publish` to publish the new version. If this fails due to authentication, inform the user to run `cargo login` first.
