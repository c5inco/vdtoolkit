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
   run never publishes a release. Tick **Sign and notarize the macOS binaries**
   to also exercise the signing job (see below).
5. Create and push the exact tag `v<version>`. The workflow rejects a tag that
   does not match `Cargo.toml`, rebuilds all artifacts, verifies conformance,
   signs and notarizes the macOS binaries, and creates the GitHub release with
   checksums.
6. Inspect the release notes and archives, then publish to crates.io when ready:

   ```sh
   cargo publish --locked
   ```

7. Verify a fresh installation and one conversion from each distribution
   channel that was published. For the macOS archives, also confirm the
   signature:

   ```sh
   codesign --verify --strict --verbose=2 vdt
   codesign -dv vdt   # expect "Authority=Developer ID Application: …"
   ```

## macOS signing and notarization

After the four builds finish, the `sign-macos` job downloads the two macOS
archives. It signs each `vdt` with a Developer ID Application certificate using
the hardened runtime and a secure timestamp, and submits both to Apple's notary
service. Then it repackages the archives, regenerates their checksums, and
replaces the unsigned artifacts. The Linux and Windows archives are unaffected.
A bare executable cannot carry a stapled ticket, so Gatekeeper confirms
notarization online the first time the binary runs.

The job runs for every tag and for manual runs with **Sign and notarize the
macOS binaries** ticked. It stops before signing, naming every missing secret,
if any of the five is unset. A tag never publishes unsigned macOS binaries.

### One-time setup

1. In the Apple Developer account, create a **Developer ID Application**
   certificate. Export it with its private key from Keychain Access as a `.p12`
   file protected by a strong password.
2. In App Store Connect, go to **Users and Access → Integrations → App Store
   Connect API**. Create a team key with the **Developer** role and download
   the `.p8` file. Apple lets you download it only once. Note the key ID and the
   issuer ID shown above the team key list.
3. Under **Settings → Environments**, create an environment named
   `macos-signing`. Under **Deployment branches and tags**, choose **Selected
   branches and tags** and add the tag rule `v*` and the branch rule `main`.
4. Add these as **environment secrets** of `macos-signing`, not as repository
   secrets:

   | Secret | Value |
   | --- | --- |
   | `MACOS_CERTIFICATE` | base64 of the `.p12` file |
   | `MACOS_CERTIFICATE_PASSWORD` | the `.p12` export password |
   | `APPLE_API_KEY` | base64 of the `.p8` file |
   | `APPLE_API_KEY_ID` | the key ID |
   | `APPLE_API_ISSUER_ID` | the issuer ID |

   `gh` sets them without the values touching the clipboard or shell history:

   ```sh
   gh secret set MACOS_CERTIFICATE --env macos-signing < <(base64 -i certificate.p12)
   gh secret set MACOS_CERTIFICATE_PASSWORD --env macos-signing   # prompts
   gh secret set APPLE_API_KEY --env macos-signing < <(base64 -i AuthKey_XXXXXXXXXX.p8)
   gh secret set APPLE_API_KEY_ID --env macos-signing
   gh secret set APPLE_API_ISSUER_ID --env macos-signing
   ```

5. Delete the local `.p12` and `.p8` copies, or move them to a password
   manager. Never commit them.
6. Test the setup from `main` with a manual run that has signing ticked, and
   check the downloaded macOS archives with `codesign`. To test from another
   branch first, add that branch to the environment's deployment rules
   temporarily and remove it afterwards.

### Keeping the secrets safe

- Only the `sign-macos` job can read the secrets, and only on `main` or a `v*`
  tag. Other branches and pull requests, including workflows from forks, cannot
  read them. The job never compiles project code. It receives the secrets only
  as step environment variables, decodes them inside `$RUNNER_TEMP`, imports the
  certificate into a temporary keychain, and deletes both when the job ends,
  even if it fails.
- If collaborators with write access join the repository, add a required
  reviewer to `macos-signing`, since they could otherwise change the workflow
  on `main` or push a `v*` tag.
- Rotate the API key in App Store Connect and revoke the certificate in the
  Developer account if either might have been exposed. Then update the secrets.
- The Developer ID certificate expires after five years. Renew it and replace
  `MACOS_CERTIFICATE` and `MACOS_CERTIFICATE_PASSWORD` before it expires.
