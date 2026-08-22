# Temp Files Cleanup Rust

\`temp_files_cleanup_rust\` is a Windows-focused automation engine and desktop application written in Rust. It lets local users define JSON automations that evaluate triggers and conditions, run system actions such as temporary-file cleanup or application launch, and record per-action results. The repository contains the Rust engine, an \`eframe\` desktop shell, and a command-line engine mode; it does not include a packaged installer or release distribution workflow.

## Overview

An automation has an ID, name, enabled state, trigger, condition tree, actions, and execution policies. Configuration is validated before loading or saving. The engine receives trigger events, checks enabled state and trigger matching, evaluates conditions, executes actions in order, and records each action result in JSONL history.

The desktop application is a local egui shell around the same model, with dashboard, automation editing, history, settings, and manual execution views. Normal desktop launch also starts the background runtime, including scheduled triggers and, on Windows, global hotkey listeners. The long-running \`--engine\` mode remains available as a headless runtime. There is no server or cloud service.

## Key Features

- JSON automation definitions with schema version validation.
- Manual, interval, daily, and Windows global-hotkey triggers.
- Empty, time-range, battery-below, AND, OR, and NOT conditions.
- Temporary-file cleanup with configured roots or Windows environment-based defaults.
- Application launching with configurable arguments.
- Windows toast notifications.
- Per-automation concurrency and failure policies.
- JSONL execution history with timestamps, action details, success state, and messages.
- Structured logging through \`tracing\` and \`tracing-subscriber\`.
- Local egui interface for creating, editing, enabling, disabling, running, and reviewing automations.
- Headless engine mode and an internal elevated-cleanup entry point.

## Architecture

\`\`\`mermaid
flowchart LR
    Config[automations.json] --> Persistence[Persistence and validation]
    Persistence --> Domain[Automation model]
    Domain --> Sources[Trigger sources]
    Sources --> Engine[Engine]
    Engine --> Conditions[Condition evaluator]
    Conditions --> Engine
    Engine --> Actions[Action executor]
    Actions --> System[Filesystem / process / Windows APIs]
    Engine --> History[JSONL history]
    GUI[eframe desktop shell] --> Persistence
    GUI --> Engine
    Scheduler[Interval and daily scheduler] --> Sources
    Hotkeys[Windows global hotkeys] --> Sources
\`\`\`

Major modules:

- \`domain\`: serializable automation, trigger, condition, action, and policy types.
- \`persistence\`: JSON loading, saving, and schema/configuration validation.
- \`triggers\`: interval and daily scheduler implementation.
- \`windows\`: Windows hotkey registration and elevation helpers.
- \`conditions\`: condition-tree evaluation and system time/battery reads.
- \`actions\`: filesystem cleanup, process launching, and notifications.
- \`engine\`: trigger matching, condition checks, policy handling, action sequencing, and logs.
- \`history\`: thread-safe append/read access to JSONL records.
- \`app\`: local egui desktop shell.
- \`main\`: desktop, headless-engine, and internal-operation entry points.

## Automation Model

\`\`\`text
Trigger event → enabled + trigger match → condition evaluation
→ concurrency policy → actions in order → JSONL history
\`\`\`

Unmatched, disabled, or condition-failing automations are skipped. \`skip_if_running\` suppresses a second dispatch while an automation is active. With \`continue\`, later actions run after a failure; \`stop\` returns the action error immediately.

## Triggers

| Type | Configuration | Behavior |
| --- | --- | --- |
| \`manual\` | \`{ "type": "manual" }\` | Runs from the desktop app or a manual trigger event. |
| \`interval\` | \`{ "type": "interval", "seconds": 3600 }\` | The engine scheduler emits an event at the configured interval. Zero is rejected. |
| \`daily\` | \`{ "type": "daily", "time_hh_mm": "09:00" }\` | The scheduler emits an event at local time; values must be valid HH:MM. |
| \`hotkey\` | \`{ "type": "hotkey", "combination": "Ctrl+Alt+T" }\` | On Windows, the engine registers a global hotkey. Modifiers include Ctrl, Shift, Alt, and Win; keys include alphanumeric keys, Space, Enter, and F1-F24. |

Scheduled and hotkey sources are started by both normal desktop launch and \`--engine\`; the desktop shell sends manual execution and configuration refresh commands to the same background runtime.

## Conditions

- \`empty\` always evaluates to true.
- \`time_range\` checks local time and supports ranges crossing midnight.
- \`battery_below\` checks Windows battery percentage; an unavailable battery reports false.
- \`and\` requires every child; \`or\` requires one child; \`not\` inverts its child.

Conditions can be nested:

\`\`\`json
{
  "type": "and",
  "children": [
    { "type": "leaf", "condition": { "type": "battery_below", "percentage": 25 } },
    { "type": "leaf", "condition": { "type": "time_range", "start_hh_mm": "18:00", "end_hh_mm": "23:00" } }
  ]
}
\`\`\`

## Actions

| Type | Required configuration | Behavior |
| --- | --- | --- |
| \`clean_temporary_files\` | Optional \`directories\` array | Removes immediate files and subdirectories under each root. With \`null\`, uses \`TEMP\`, \`%WINDIR%/Temp\`, and \`%WINDIR%/Prefetch\` when available. Missing roots are ignored and protected entries are skipped with warnings. |
| \`launch_application\` | \`executable\`; optional \`args\` | Starts the executable with supplied arguments. |
| \`show_notification\` | \`title\`, \`message\` | Shows a Windows toast notification; it errors on non-Windows targets. |

## Configuration

Configuration is JSON with \`schema_version\` currently required to be \`1\` and an \`automations\` array. IDs must be non-empty and unique, and each automation must have at least one action. See [automations.json.example](automations.json.example).

\`\`\`json
{
  "schema_version": 1,
  "automations": [
    {
      "id": "manual-clean",
      "name": "Clean temporary files",
      "enabled": true,
      "trigger": { "type": "manual" },
      "conditions": { "type": "empty" },
      "actions": [
        { "type": "clean_temporary_files", "directories": ["C:/Users/REPLACE_ME/AppData/Local/Temp"] },
        { "type": "show_notification", "title": "Cleanup", "message": "Temporary-file cleanup completed." }
      ],
      "settings": { "concurrency_policy": "skip_if_running", "failure_policy": "continue" }
    }
  ]
}
\`\`\`

\`concurrency_policy\` is \`allow\` or \`skip_if_running\`; \`failure_policy\` is \`continue\` or \`stop\`. The cleanup action retains each configured root and removes its contents.

## Execution History

History is appended as one JSON object per line to \`execution.jsonl\` under the application data directory resolved by \`directories\` (\`com/temp-files-cleanup/engine\`). Each record contains a Unix timestamp, automation ID, debug-formatted action, success boolean, and result/error message. The desktop history view reads these records and supports text filtering.

## CLI Usage

The executable defaults to the desktop application. The configuration path defaults to \`automations.json\`.

\`\`\`text
temp_files_cleanup_rust.exe [CONFIG_PATH]
temp_files_cleanup_rust.exe --engine [CONFIG_PATH]
temp_files_cleanup_rust.exe --internal-elevated-clean
\`\`\`

Examples:

\`\`\`powershell
.\temp_files_cleanup_rust.exe .\automations.json
.\temp_files_cleanup_rust.exe --engine .\automations.json
.\temp_files_cleanup_rust.exe --internal-elevated-clean
\`\`\`

The first form opens the desktop application and starts its background runtime. \`--engine\` runs the same long-lived scheduler/event-loop mode without the GUI. A missing configuration file is initialized with an empty schema-version-1 configuration. \`--internal-elevated-clean\` runs default cleanup and is intended for the Windows elevation helper, not arbitrary-path input.

## Windows Integration

Windows is the current target platform. The implementation uses Windows API bindings for local time, battery status, global hotkeys, message-loop handling, and ShellExecute elevation. \`tauri-winrt-notification\` provides toast notifications. Cleanup defaults derive from \`TEMP\` and \`WINDIR\`.

The repository does not claim Linux or macOS support. Some non-Windows paths exist for compilation and tests, but the application target is Windows.

## Project Structure

\`\`\`text
.
├── automations.json.example  # Example schema-1 configuration
├── Cargo.toml                # Package and dependency manifest
├── src/
│   ├── main.rs               # Application entry points
│   ├── app.rs                # egui desktop shell
│   ├── domain.rs             # Automation model
│   ├── engine.rs             # Event dispatch and policies
│   ├── persistence.rs        # JSON persistence and validation
│   ├── triggers.rs           # Interval and daily scheduling
│   ├── conditions.rs         # Condition evaluation
│   ├── actions.rs            # System actions
│   ├── history.rs            # JSONL history
│   ├── windows.rs            # Windows hotkeys/elevation
│   ├── registry.rs            # Action/trigger metadata
│   └── errors.rs              # Engine errors
└── README.md
\`\`\`

## Technology Stack

- Rust, edition 2024, built with Cargo.
- \`serde\` and \`serde_json\` for JSON configuration/history.
- \`thiserror\` for typed errors.
- \`tracing\` and \`tracing-subscriber\` for structured logs.
- \`eframe\`/egui for the desktop interface.
- \`directories\` for application data paths.
- \`windows\` for Windows API bindings.
- \`tauri-winrt-notification\` for Windows toast notifications.
- Standard-library filesystem, process, threading, and channel APIs.

## Requirements

- Windows is the supported application platform.
- A Rust toolchain and Cargo capable of building Rust edition 2024 projects.
- The Windows build must resolve the Windows API and notification dependencies declared in \`Cargo.toml\`.
- No external service, database, or runtime server is required.

The project does not include an installer or prebuilt release binary.

## Installation

> **Installation instructions will be added here.**
>
> The project is currently under active development and the final Windows installation/distribution workflow is being prepared.
>
> This section is reserved for prerequisites, installation, building from source, running the application, and release binaries/installers.

## Usage

1. Copy [automations.json.example](automations.json.example) to a working configuration path.
2. Replace placeholder paths and adjust triggers, conditions, actions, and policies.
3. Start the desktop interface for editing, manual execution, scheduled triggers, and Windows hotkey events, or start \`--engine\` for headless operation.
4. Review \`execution.jsonl\` in the application data directory for action results.

The cleanup action removes the contents of its configured roots. Review paths carefully before enabling an automation.

## Development

From the repository root:

\`\`\`bash
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features
\`\`\`

The source includes unit tests for condition-tree logic, time parsing, cleanup behavior, and daily-trigger parsing. It does not currently include an end-to-end test suite or release packaging workflow.

## Logging and Error Handling

The binary initializes \`tracing-subscriber\` with an \`info\` environment filter. The engine reports operational details through structured logs; recoverable cleanup-entry failures are warned and counted. Configuration, I/O, invalid-time, validation, action, and trigger-channel failures are represented by \`EngineError\`.

## Current Status and Scope

Implemented today: the automation model, JSON persistence/validation, conditions, actions, JSONL history, desktop shell with a background runtime, headless scheduler/event loop, Windows hotkeys, notifications, and elevation helper.

Not included today: a packaged installer, release binaries, cloud synchronization, a service/daemon installer, or a finalized Windows distribution workflow. Future product/UI work should be treated as planned until implemented in the repository.

## License

No license file or license declaration is currently present in the repository.
