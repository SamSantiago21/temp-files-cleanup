# Temp Files Cleanup Rust

Event-driven automation engine for Windows temporary-file cleanup, rewritten in
Rust from the original hotkey-triggered Python script (`cleanup_automation.py`,
preserved in the repository history).

Build with the Windows Rust MSVC toolchain and Visual Studio C++ Build Tools:

```text
cargo run -- automations.json
```

If the configuration file does not exist, the engine creates an empty one and waits for events. Copy `automations.json.example` and edit its paths to configure automations.

Implemented boundaries:

- serde JSON configuration with schema versioning;
- explicit `And`/`Or`/`Not`/`Leaf` condition trees;
- pluggable JSONL execution history;
- cleanup, process-launch, and notification action executors;
- blocking `mpsc` execution loop and interval trigger threads;
- registry definitions for future GUI discovery;
- Windows-specific integration isolated in `src/windows.rs`.

The current environment cannot link the executable because Visual Studio's `link.exe` is not installed. `cargo fmt` passes; run `cargo test` after installing the Windows C++ build tools.
