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
- blocking `mpsc` execution loop with one scheduler thread for interval and daily triggers;
- one Win32 global-hotkey message-loop thread, including validated Ctrl/Alt/Shift combinations;
- local Windows-time daily scheduling and BatteryBelow evaluation;
- Windows toast notifications and a controlled internal elevated cleanup operation;
- registry definitions for future GUI discovery;
- Windows-specific integration isolated in `src/windows.rs`.

Windows-specific runtime behavior (hotkeys, UAC, toast delivery, and protected-file cleanup)
must be verified on Windows with the required C++/Windows SDK tools. Tests in this checkout
could not run because `dlltool.exe` is unavailable in the current environment.
