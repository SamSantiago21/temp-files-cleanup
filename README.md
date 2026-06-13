# Windows Temp Cleanup Automation

A background Python script that cleans temporary system files via hotkey trigger.

## Features

- Cleans three folders on demand:
  1. `C:\Windows\Prefetch` - Windows prefetch files
  2. `C:\Windows\Temp` - Windows temp directory
  3. `%TEMP%` - User's temp directory (resolved automatically)
- Silent operation - skips locked/in-use files without errors or popups
- Logs all actions with timestamps to `cleanup_log.txt` in your home directory
- Runs in background until manually stopped

## Requirements

- Python 3.6+
- Windows OS (requires Administrator privileges for full functionality)

## Installation

Install the required dependency:

```bash
pip install keyboard
```

Or install from the included requirements.txt:

```bash
pip install -r requirements.txt
```

## Running the Script

The script **automatically requests Administrator privileges** via a UAC prompt when launched. No manual "Run as administrator" step is needed — just run:

```bash
python cleanup_automation.py
```

If you decline the UAC prompt, the script will exit with an error because the `keyboard` library requires elevated privileges to register global hotkeys on Windows.

## Hotkey Usage

Once started, the script listens for these global hotkeys:

| Hotkey | Action |
|--------|--------|
| `Alt + Shift + 0` | Clean all three temporary folders |
| `Ctrl + Alt + Shift + Q` | Quit the script cleanly |

## Log File

All cleanup actions are logged to `cleanup_log.txt` in your user home directory (e.g., `C:\Users\<username>\cleanup_log.txt`).

Each log entry includes:
- Timestamp
- Which folder was processed
- Number of items deleted and skipped

## Example Log Output

```
[2026-06-11 14:30:15] Cleanup started
[2026-06-11 14:30:15] Prefetch - Deleted: 42, Skipped: 3
[2026-06-11 14:30:20] Windows Temp - Deleted: 15, Skipped: 0
[2026-06-11 14:30:25] User Temp - Deleted: 8, Skipped: 2
[2026-06-11 14:30:25] Cleanup completed - Total deleted: 65, Total skipped: 5
```