# Release checklist

Releases publish native archives on GitHub. Publishing the Rust package to
crates.io remains an explicit, separate maintainer action.

1. Update the version in `Cargo.toml` and `Cargo.lock`, then finalize
   `CHANGELOG.md` with the same version and date.
2. From a clean checkout, run:

   ```sh
   cargo +1.85.0 test --locked
   bash tools/accept-v1.sh
   cargo package --locked
   ```

3. If VectorDrawable structure or API qualification changed, rerun the optional
   Android renderer harness and record the matrix in
   `tools/android-renderer/RESULTS.md`.
4. Push the release commit and manually run **Release binaries**. Confirm the
   exhaustive conformance job and all four native artifact builds pass. A manual
   run never publishes a release.
5. Create and push the exact tag `v<version>`. The workflow rejects a tag that
   does not match `Cargo.toml`, rebuilds all artifacts, verifies conformance, and
   creates the GitHub release with checksums.
6. Inspect the release notes and archives, then publish to crates.io when ready:

   ```sh
   cargo publish --locked
   ```

7. Verify a fresh installation and one conversion from each distribution
   channel that was published.
