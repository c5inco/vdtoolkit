#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
python3 tools/check-material-symbols.py
python3 tools/check-material-symbols.py --suite twotone-100
cargo build --release --locked

install_root=$(mktemp -d)
trap 'rm -rf "$install_root" "${package_root:-}"' EXIT
cargo install --path . --locked --root "$install_root"
"$install_root/bin/vdt" --version

version=$(target/release/vdt --version | awk '{print $2}')
target_name=$(rustc -vV | awk '/^host:/ {print $2}')
package="vdtoolkit-v${version}-${target_name}"
destination="target/release-dist"
rm -rf "$destination"
mkdir -p "$destination/$package"
cp target/release/vdt README.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE \
  "$destination/$package/"
tar -C "$destination" -czf "$destination/$package.tar.gz" "$package"
sha256sum "$destination/$package.tar.gz" > "$destination/$package.tar.gz.sha256"
sha256sum -c "$destination/$package.tar.gz.sha256"

package_root=$(mktemp -d)
tar -C "$package_root" -xzf "$destination/$package.tar.gz"
"$package_root/$package/vdt" --version

if command -v ldd >/dev/null 2>&1; then
  if ldd target/release/vdt | grep -Eiq 'android|java|jvm'; then
    echo "release executable unexpectedly links an Android/JVM library" >&2
    exit 1
  fi
fi

echo
echo "V1 acceptance passed"
echo "Package: $destination/$package.tar.gz"
echo "Checksum: $destination/$package.tar.gz.sha256"
