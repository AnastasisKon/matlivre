#!/usr/bin/env bash
# Builds MatLivre on Linux and packs it as dist/matlivre-linux-x86_64.tar.gz.
# Build dependencies (Ubuntu/Debian): build-essential pkg-config libgtk-3-dev libxkbcommon-dev libwayland-dev
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --release
out=dist/matlivre-linux-$(uname -m)
rm -rf "$out" && mkdir -p "$out"
cp target/release/matlivre target/release/matlivre-cli "$out/"
cp tools/linux/matlivre.desktop "$out/"
cp crates/livre-app/assets/matlivre.svg "$out/"
cat > "$out/install.sh" <<'INSTALL'
#!/usr/bin/env bash
# Installs for the current user: ~/.local/bin and the application menu.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
install -Dm755 "$here/matlivre" "$HOME/.local/bin/matlivre"
install -Dm755 "$here/matlivre-cli" "$HOME/.local/bin/matlivre-cli"
install -Dm644 "$here/matlivre.svg" "$HOME/.local/share/icons/hicolor/scalable/apps/matlivre.svg"
install -Dm644 "$here/matlivre.desktop" "$HOME/.local/share/applications/matlivre.desktop"
echo "Installed. Make sure ~/.local/bin is on your PATH."
INSTALL
chmod +x "$out/install.sh"
tar -C dist -czf "$out.tar.gz" "$(basename "$out")"
echo "$out.tar.gz"
