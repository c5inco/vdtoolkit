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
   channel that was published. For the macOS archives, also confirm the
   signature:

   ```sh
   codesign --verify --strict --verbose=2 vdt
   codesign -dv vdt   # expect "Authority=Developer ID Application: …"
   ```

## macOS signing and notarization

The two macOS builds sign `vdt` with a Developer ID Application certificate
using the hardened runtime and a secure timestamp, then submit it to Apple's
notary service. The Linux and Windows archives are unaffected. A bare
executable cannot carry a stapled ticket, so Gatekeeper confirms notarization
online the first time the binary runs.

Tagged releases fail if the signing secrets are missing. A manual run without
them builds unsigned macOS binaries and prints a warning. With the secrets set,
a manual run signs and notarizes, which is the way to test the setup before
tagging.

### One-time setup

1. In the Apple Developer account, create a **Developer ID Application**
   certificate. Export it with its private key from Keychain Access as a `.p12`
   file protected by a strong password.
2. In App Store Connect, go to **Users and Access → Integrations → App Store
   Connect API**. Create a team key with the **Developer** role and download
   the `.p8` file. Apple lets you download it only once. Note the key ID and the
   issuer ID.
3. Add these repository secrets under **Settings → Secrets and variables →
   Actions**:

   | Secret | Value |
   | --- | --- |
   | `MACOS_CERTIFICATE` | `base64 -i certificate.p12 \| pbcopy` |
   | `MACOS_CERTIFICATE_PASSWORD` | the `.p12` export password |
   | `APPLE_API_KEY` | `base64 -i AuthKey_XXXXXXXXXX.p8 \| pbcopy` |
   | `APPLE_API_KEY_ID` | the key ID |
   | `APPLE_API_ISSUER_ID` | the issuer ID |

   Alternatively, `gh secret set MACOS_CERTIFICATE < <(base64 -i certificate.p12)`
   sets a secret without using the clipboard.
4. Delete the local `.p12` and `.p8` copies, or move them to a password
   manager. Never commit them. The workflow decodes them only inside
   `$RUNNER_TEMP`, imports the certificate into a temporary keychain, and
   deletes both when the job ends, even if it fails.

### Keeping the secrets safe

- The release workflow runs only on `v*` tag pushes and manual dispatch. It is
  never triggered by pull requests, so workflows from forks cannot read these
  secrets.
- Anyone with write access can edit a workflow on a branch and read repository
  secrets. For stricter control, move the five secrets to an environment that
  only allows `v*` tags and requires a reviewer, and add `environment:` to the
  build job.
- Rotate the API key in App Store Connect and revoke the certificate in the
  Developer account if either might have been exposed. Then update the secrets.
- The Developer ID certificate expires after five years. Renew it and replace
  `MACOS_CERTIFICATE` and `MACOS_CERTIFICATE_PASSWORD` before it expires.
