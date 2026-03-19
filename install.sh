#!/usr/bin/env bash
set -euo pipefail

BINARY_DIR="$HOME/.claude/bin"
SETTINGS_FILE="$HOME/.claude/settings.json"
BINARY_NAME="claude-perm-router"

echo "Building claude-perm-router (release)..."
cargo build --release

echo "Installing to $BINARY_DIR/$BINARY_NAME..."
mkdir -p "$BINARY_DIR"
cp "target/release/$BINARY_NAME" "$BINARY_DIR/$BINARY_NAME"

# Check if hook is already configured
if [ -f "$SETTINGS_FILE" ]; then
    if grep -q "$BINARY_NAME" "$SETTINGS_FILE" 2>/dev/null; then
        echo "Hook already configured in $SETTINGS_FILE"
        echo "Done!"
        exit 0
    fi
fi

echo ""
echo "Binary installed. To activate, add this hook to $SETTINGS_FILE:"
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
echo ""
echo "Or if you already have hooks configured, add the PreToolUse entry to your existing hooks object."
echo ""
read -p "Add hook to $SETTINGS_FILE automatically? [y/N] " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    if [ ! -f "$SETTINGS_FILE" ]; then
        # No settings file — create one with just the hook
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
        exit 1
    fi
fi

echo "Done!"
