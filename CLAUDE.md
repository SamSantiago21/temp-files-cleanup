# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A single-script Windows background utility that cleans temporary system files via global hotkeys. The script runs as a persistent process, listening for hotkey presses to trigger cleanup of `C:\Windows\Prefetch`, `C:\Windows\Temp`, and `%TEMP%`.

## Commands

```bash
# Install dependencies
pip install -r requirements.txt

# Run the script (auto-elevates via UAC)
python cleanup_automation.py

# Run without console window (for background/scheduled use)
pythonw cleanup_automation.py
```

There are no tests, linting configuration, or build steps in this project.

## Architecture

All logic lives in a single file: `cleanup_automation.py`.

### Flow

1. **Privilege check** — `is_admin()` tests for Administrator rights. If not elevated, `elevate()` re-launches via `ShellExecuteW` with the `"runas"` verb (UAC prompt). The non-elevated instance then exits.
2. **Hotkey registration** — `keyboard.add_hotkey()` binds two callbacks:
   - `Alt+Shift+0` → `run_cleanup()`
   - `Ctrl+Alt+Shift+Q` → `_request_quit()`
3. **Cleanup** — `run_cleanup()` iterates over three folders in order (Prefetch → Windows Temp → User Temp), calling `clean_folder()` for each. That helper walks the directory, deletes files with `os.remove` and directories with `shutil.rmtree`, and silently skips locked/in-use items. Results are logged and a summary is printed to console.
4. **Shutdown** — The main loop blocks on a `threading.Event` (`_quit_event`) rather than `keyboard.wait()`, so the quit callback can unblock it cleanly. On exit, `keyboard.unhook_all()` removes all hooks.

### Key Design Decisions

- **Single-file script** — no package structure, no config files. All target paths are hard-coded.
- **Silent error handling** — locked or in-use files are skipped without user notification, matching the "silent operation" design goal.
- **`threading.Event` for shutdown** — avoids the blocking `keyboard.wait()` pattern so the quit hotkey can terminate the process cleanly.
- **Logging** — plain-text append-only log at `~/cleanup_log.txt` with timestamps. No rotation or external logging framework.

### Dependencies

- `keyboard>=0.13.0` — the only external package; used for global hotkey hooks on Windows.

### Startup Automation

The README documents a full Windows Task Scheduler setup (batch file launcher + "At log on" trigger with highest privileges) for running the script automatically at user login. See README.md for step-by-step instructions.
