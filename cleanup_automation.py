# -*- coding: utf-8 -*-
"""
Cleanup Automation Script for Windows
====================================
This script automatically requests Administrator privileges via UAC if needed.
The ``keyboard`` library requires elevated rights to install global hotkey hooks.

Hotkeys:
- Alt+Shift+0: Clean temporary folders (Prefetch, Windows Temp, User Temp)
- Ctrl+Alt+Shift+Q: Quit the script cleanly
"""

import ctypes
import os
import shutil
import sys
import threading
import keyboard
from datetime import datetime
from pathlib import Path


LOG_FILE = Path.home() / "cleanup_log.txt"

# Threading event used for a clean shutdown
_quit_event = threading.Event()


# ──────────────────────────────────────────────
#  Privilege helpers
# ──────────────────────────────────────────────

def is_admin() -> bool:
    """Return True if the current process has Administrator privileges."""
    try:
        return ctypes.windll.shell32.IsUserAnAdmin() != 0
    except (AttributeError, OSError):
        return False


def elevate() -> None:
    """Re-launch the current script with Administrator rights via UAC.

    If the user declines the UAC prompt the script exits with an error
    message instead of running without privileges (which would silently
    fail to register hotkeys).
    """
    # ShellExecuteW returns an HINSTANCE > 32 on success
    result = ctypes.windll.shell32.ShellExecuteW(
        None,                       # parent window handle
        "runas",                    # verb – request elevation
        sys.executable,             # program – the Python interpreter
        " ".join(sys.argv),         # parameters – this script + args
        None,                       # working directory (inherit)
        1,                          # SW_SHOWNORMAL
    )
    if result <= 32:
        print("ERROR: UAC elevation was declined or failed. "
              "The script cannot register global hotkeys without "
              "Administrator privileges.")
        sys.exit(1)
    # The elevated copy is now running; exit this (non-elevated) copy.
    sys.exit(0)


# ──────────────────────────────────────────────
#  Logging
# ──────────────────────────────────────────────

def log_message(message: str) -> None:
    """
    Write a timestamped message to the log file.

    Args:
        message: The message to log
    """
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    log_entry = f"[{timestamp}] {message}\n"
    try:
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(log_entry)
    except OSError:
        # If we can't write the log, print to console as a fallback
        print(f"  (log write failed) {log_entry.strip()}")


# ──────────────────────────────────────────────
#  Folder cleaning
# ──────────────────────────────────────────────

def clean_folder(folder_path: str) -> tuple[int, int]:
    """
    Clean a folder by deleting all files and subfolders.
    Silently skips any items that cannot be deleted.

    Args:
        folder_path: Path to the folder to clean

    Returns:
        Tuple of (items_deleted, items_skipped)
    """
    deleted = 0
    skipped = 0

    # Skip if folder doesn't exist
    if not os.path.exists(folder_path):
        log_message(f"Folder does not exist: {folder_path}")
        return deleted, skipped

    # Process all items in the folder
    try:
        items = list(os.listdir(folder_path))
    except (PermissionError, OSError) as e:
        log_message(f"Cannot access folder {folder_path}: {e}")
        return deleted, skipped

    for item in items:
        item_path = os.path.join(folder_path, item)

        try:
            if os.path.isfile(item_path) or os.path.islink(item_path):
                os.remove(item_path)
                deleted += 1
            elif os.path.isdir(item_path):
                shutil.rmtree(item_path)
                deleted += 1
        except (PermissionError, OSError, shutil.Error):
            # Item is locked, in use, or other deletion error - skip silently
            skipped += 1

    return deleted, skipped


def clean_prefetch() -> tuple[int, int]:
    """Clean the Windows Prefetch folder."""
    return clean_folder(r"C:\Windows\Prefetch")


def clean_windows_temp() -> tuple[int, int]:
    """Clean the Windows Temp folder."""
    return clean_folder(r"C:\Windows\Temp")


def clean_user_temp() -> tuple[int, int]:
    """Clean the user's temp folder."""
    return clean_folder(os.environ.get("TEMP", ""))


def run_cleanup() -> None:
    """
    Run the full cleanup process and log results.
    Cleans all three folders in order and prints a summary.
    """
    log_message("Cleanup started")
    print("\nRunning cleanup...")

    total_deleted = 0
    total_skipped = 0

    # Clean Prefetch
    deleted, skipped = clean_prefetch()
    total_deleted += deleted
    total_skipped += skipped
    log_message(f"Prefetch - Deleted: {deleted}, Skipped: {skipped}")

    # Clean Windows Temp
    deleted, skipped = clean_windows_temp()
    total_deleted += deleted
    total_skipped += skipped
    log_message(f"Windows Temp - Deleted: {deleted}, Skipped: {skipped}")

    # Clean User Temp
    deleted, skipped = clean_user_temp()
    total_deleted += deleted
    total_skipped += skipped
    log_message(f"User Temp - Deleted: {deleted}, Skipped: {skipped}")

    log_message(f"Cleanup completed - Total deleted: {total_deleted}, Total skipped: {total_skipped}")

    # Print short summary to console
    print(f"Cleanup complete - {total_deleted} items deleted, {total_skipped} items skipped\n")


# ──────────────────────────────────────────────
#  Quit handler
# ──────────────────────────────────────────────

def _request_quit() -> None:
    """Signal the main thread to exit cleanly."""
    _quit_event.set()


# ──────────────────────────────────────────────
#  Main
# ──────────────────────────────────────────────

def main() -> None:
    """
    Main entry point - checks privileges, sets up hotkeys, and runs the
    listener loop.
    """
    # ── Step 1: Ensure we are running as Administrator ──
    if not is_admin():
        print("Not running as Administrator – requesting elevation via UAC...")
        elevate()
        # elevate() calls sys.exit(); execution never reaches here.

    # ── Step 2: Print startup banner ──
    print("=" * 50)
    print("Cleanup Automation Script Active  (Administrator)")
    print("=" * 50)
    print("Hotkeys:")
    print("  - Alt+Shift+0      : Clean temporary folders")
    print("  - Ctrl+Alt+Shift+Q : Quit script")
    print("=" * 50)

    # ── Step 3: Register hotkeys ──
    try:
        keyboard.add_hotkey("alt+shift+0", run_cleanup)
        keyboard.add_hotkey("ctrl+alt+shift+q", _request_quit)
    except Exception as e:
        print(f"ERROR: Failed to register hotkeys: {e}")
        print("Make sure no other program is blocking keyboard hooks.")
        log_message(f"Hotkey registration failed: {e}")
        sys.exit(1)

    log_message("Cleanup automation script started - listener active (Administrator)")
    print("\nListening for hotkeys... (press Ctrl+Alt+Shift+Q to quit)\n")

    # ── Step 4: Wait for quit signal ──
    # Using a threading.Event instead of keyboard.wait() so the quit
    # hotkey callback can cleanly unblock the main thread.
    try:
        _quit_event.wait()
    except KeyboardInterrupt:
        pass
    finally:
        keyboard.unhook_all()

    log_message("Cleanup automation script stopped by user")
    print("Script stopped. Goodbye!")


if __name__ == "__main__":
    main()