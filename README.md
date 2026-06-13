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

## Running Automatically on Startup (No Manual Launch Needed)

By default, this script must be run manually each time. To have it run silently 
in the background every time you log into Windows — so the hotkey is always 
available — set it up as a scheduled task.

### Step 1: Create a launcher batch file

In the project folder, create a file named `run_cleanup.bat`:

```bat
@echo off
cd /d "D:\path\to\PA_1"
pythonw cleanup_automation.py
```

Replace `D:\path\to\PA_1` with the actual path to this project on your machine.  
`pythonw` runs the script without opening a console window.

### Step 2: Open Task Scheduler

Press `Win + R`, type `taskschd.msc`, and press Enter.

### Step 3: Create the task

- Click **Create Task** (not "Create Basic Task")
- **General tab:**
  - Name: `PA1 Cleanup Hotkey`
  - Check **"Run with highest privileges"**
  - Configure for: your Windows version

### Step 4: Set the trigger

- **Triggers tab → New**
- Begin the task: **At log on**
- Select **Specific user** → your account
- Click OK

### Step 5: Set the action

- **Actions tab → New**
- Action: **Start a program**
- Browse to `run_cleanup.bat`
- Click OK

### Step 6: Adjust conditions (for laptops)

- **Conditions tab**
- Uncheck **"Start the task only if the computer is on AC power"**

### Step 7: Save

- Click OK
- Enter your Windows account password when prompted (required for highest privileges)

### Step 8: Verify

- Right-click the task → **Run**
- Open Task Manager and confirm `pythonw.exe` is running
- Press **Alt+Shift+0**, then check `cleanup_log.txt` in your home folder for a new entry

### Step 9: Confirm after reboot

- Restart your laptop and log in normally
- Press **Alt+Shift+0** without opening anything manually
- A new entry in `cleanup_log.txt` confirms the listener is running automatically

Once set up, the hotkey is always active in the background — no manual startup required.