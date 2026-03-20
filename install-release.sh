#!/usr/bin/env bash
set -euo pipefail

REPO="kcberg/claude-perm-router"
BINARY_DIR="$HOME/.claude/bin"
BINARY_NAME="claude-perm-router"
SETTINGS_FILE="$HOME/.claude/settings.json"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin)
        case "$ARCH" in
            arm64) ARTIFACT="claude-perm-router-macos-aarch64" ;;
            *) echo "Unsupported macOS architecture: $ARCH (only Apple Silicon is supported)"; exit 1 ;;
        esac
        ;;
    Linux)
        case "$ARCH" in
            x86_64)  ARTIFACT="claude-perm-router-linux-x86_64" ;;
            aarch64) ARTIFACT="claude-perm-router-linux-aarch64" ;;
            *) echo "Unsupported Linux architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

# Get latest release URL
echo "Detecting latest release..."
DOWNLOAD_URL=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep "browser_download_url.*$ARTIFACT" \
    | head -1 \
    | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: Could not find release artifact '$ARTIFACT'"
    echo "Check https://github.com/$REPO/releases for available downloads"
    exit 1
fi

echo "Downloading $ARTIFACT..."
mkdir -p "$BINARY_DIR"
curl -sL "$DOWNLOAD_URL" -o "$BINARY_DIR/$BINARY_NAME"
chmod +x "$BINARY_DIR/$BINARY_NAME"

echo "Installed to $BINARY_DIR/$BINARY_NAME"

# Check if hook is already configured
if [ -f "$SETTINGS_FILE" ] && grep -q "$BINARY_NAME" "$SETTINGS_FILE" 2>/dev/null; then
    echo "Hook already configured in $SETTINGS_FILE"
    echo "Done!"
    exit 0
fi

echo ""
echo "To activate, add this hook to $SETTINGS_FILE:"
echo ""
cat <<'HOOK'
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "~/.claude/bin/claude-perm-router",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
HOOK
# Only prompt interactively if stdin is a terminal (not piped from curl)
if [ -t 0 ]; then
    echo ""
    read -p "Add hook to $SETTINGS_FILE automatically? [y/N] " -n 1 -r
    echo ""

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if [ ! -f "$SETTINGS_FILE" ]; then
            cat > "$SETTINGS_FILE" <<'EOF'
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "~/.claude/bin/claude-perm-router",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
EOF
            echo "Created $SETTINGS_FILE with hook configuration."
        else
            echo "Cannot safely auto-merge into existing $SETTINGS_FILE."
            echo "Please add the hook configuration manually."
        fi
    fi
fi

echo "Done!"
