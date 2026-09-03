# Yggdrasill

A terminal user interface for managing Git branches and worktrees.

# Configuration

The optional configuration file is read from `$XDG_CONFIG_HOME/yggdrasill/config.toml`.
When `XDG_CONFIG_HOME` is not set, `$HOME/.config/yggdrasill/config.toml` is used.

See [config.example.toml](config.example.toml) for the available settings. Copy it
to the configuration path and uncomment the settings you want to enable.

For example:

```bash
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/yggdrasill"
cp config.example.toml "${XDG_CONFIG_HOME:-$HOME/.config}/yggdrasill/config.toml"
```

# Test

```bash
cargo test
cargo test -- --ignored
```
