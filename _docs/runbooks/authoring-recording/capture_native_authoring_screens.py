#!/usr/bin/env python3
"""Capture native author-mode screenshots for the Arenic runbook.

Run `cargo run -p arenic --features author` first. The script targets the most
recent `target/debug/arenic` process, drives its window with AppleScript, and
crops the window from a full-screen capture using ImageMagick.
"""

from __future__ import annotations

import re
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "_docs" / "runbooks" / "authoring-recording" / "screenshots"
FULL = OUT / "_native-full.png"


def run(command: list[str], *, text: bool = True) -> str:
    return subprocess.check_output(command, cwd=ROOT, text=text).strip()


def sh(command: str) -> str:
    return subprocess.check_output(command, cwd=ROOT, shell=True, text=True).strip()


def osascript(script: str) -> str:
    return run(["osascript", "-e", script])


def latest_arenic_pid() -> int:
    out = sh("pgrep -n -x arenic")
    return int(out)


PID = latest_arenic_pid()


def activate() -> None:
    osascript(
        f'tell application "System Events" to set frontmost of '
        f'(first process whose unix id is {PID}) to true'
    )
    time.sleep(0.25)


def window_rect() -> tuple[int, int, int, int]:
    script = (
        'tell application "System Events"\n'
        f'  tell (first process whose unix id is {PID})\n'
        '    set p to position of window 1\n'
        '    set s to size of window 1\n'
        '    return (item 1 of p as text) & "," & (item 2 of p as text) & "," & '
        '(item 1 of s as text) & "," & (item 2 of s as text)\n'
        '  end tell\n'
        'end tell'
    )
    x, y, w, h = [int(float(part)) for part in osascript(script).split(",")]
    return x, y, w, h


def desktop_points() -> tuple[int, int]:
    bounds = osascript('tell application "Finder" to get bounds of window of desktop')
    nums = [int(n) for n in re.findall(r"-?\d+", bounds)]
    return nums[2] - nums[0], nums[3] - nums[1]


def full_capture() -> tuple[list[int], tuple[int, int]]:
    OUT.mkdir(parents=True, exist_ok=True)
    subprocess.check_call(["screencapture", "-x", str(FULL)], cwd=ROOT)
    info = run(["sips", "-g", "pixelWidth", "-g", "pixelHeight", str(FULL)])
    pixels = [int(n) for n in re.findall(r"pixel(?:Width|Height): (\d+)", info)]
    if len(pixels) != 2:
        raise RuntimeError(f"Could not read screenshot size: {info}")
    return pixels, desktop_points()


def display_scale() -> tuple[float, float]:
    if not FULL.exists():
        full_capture()
    info = run(["sips", "-g", "pixelWidth", "-g", "pixelHeight", str(FULL)])
    pixels = [int(n) for n in re.findall(r"pixel(?:Width|Height): (\d+)", info)]
    point_w, point_h = desktop_points()
    return pixels[0] / point_w, pixels[1] / point_h


def screenshot(name: str) -> None:
    activate()
    pixels, (point_w, point_h) = full_capture()
    scale_x = pixels[0] / point_w
    scale_y = pixels[1] / point_h
    x, y, w, h = window_rect()
    crop = (
        f"{round(w * scale_x)}x{round(h * scale_y)}+"
        f"{round(x * scale_x)}+{round(y * scale_y)}"
    )
    out = OUT / f"{name}.png"
    subprocess.check_call(["convert", str(FULL), "-crop", crop, "+repage", str(out)], cwd=ROOT)
    print(out)


def click_content(x: int, y: int) -> None:
    activate()
    wx, wy, _, _ = window_rect()
    sx, sy = display_scale()
    # macOS window coordinates include the titlebar; Bevy content starts below it.
    hid_click(round((wx + x) * sx), round((wy + 28 + y) * sy))
    time.sleep(0.4)


KEY_CODES = {
    "b": 11,
    "d": 2,
    "e": 14,
    "n": 45,
    "o": 31,
    "p": 35,
    "r": 15,
    "t": 17,
    "w": 13,
    "]": 30,
    ".": 47,
    "/": 44,
    "3": 20,
}


def swift_event(source: str) -> None:
    subprocess.check_call(["swift", "-e", source], cwd=ROOT)


def hid_click(x: int, y: int) -> None:
    source = (
        "import CoreGraphics; import Foundation; "
        f"let p=CGPoint(x:{x},y:{y}); "
        "CGEvent(mouseEventSource:nil, mouseType:.mouseMoved, mouseCursorPosition:p, mouseButton:.left)?.post(tap:.cghidEventTap); "
        "usleep(80000); "
        "CGEvent(mouseEventSource:nil, mouseType:.leftMouseDown, mouseCursorPosition:p, mouseButton:.left)?.post(tap:.cghidEventTap); "
        "usleep(80000); "
        "CGEvent(mouseEventSource:nil, mouseType:.leftMouseUp, mouseCursorPosition:p, mouseButton:.left)?.post(tap:.cghidEventTap);"
    )
    swift_event(source)


def key(text: str, *, shift: bool = False) -> None:
    key_code(KEY_CODES[text], shift=shift)


def key_code(code: int, *, shift: bool = False) -> None:
    activate()
    flags = ".maskShift" if shift else "[]"
    source = (
        "import CoreGraphics; import Foundation; "
        "let src=CGEventSource(stateID:.hidSystemState); "
        f"let flags: CGEventFlags = {flags}; "
        f"let down=CGEvent(keyboardEventSource:src, virtualKey:CGKeyCode({code}), keyDown:true); "
        "down?.flags=flags; down?.post(tap:.cghidEventTap); "
        "usleep(70000); "
        f"let up=CGEvent(keyboardEventSource:src, virtualKey:CGKeyCode({code}), keyDown:false); "
        "up?.flags=flags; up?.post(tap:.cghidEventTap);"
    )
    swift_event(source)
    time.sleep(0.25)


def main() -> None:
    activate()
    if "--from-intro" not in sys.argv:
        screenshot("01-title-screen")
        click_content(640, 456)
        time.sleep(2.5)
    else:
        time.sleep(2.5)
    screenshot("02-overworld-author-mode")

    key("d")
    screenshot("03-heroic-difficulty")
    key("d")
    screenshot("04-mythic-difficulty")
    key("d")  # back to Normal for the rest of the walkthrough

    key("]")
    key("p")
    time.sleep(0.5)
    key("b")
    screenshot("05-boss-possession")

    key_code(122)  # F1
    screenshot("06-dope-sheet-help-overlay")
    key_code(122)  # F1

    key("e")
    screenshot("07-entity-browser")
    key_code(53)  # Esc

    key("t")
    screenshot("08-tile-editor-open")
    key("n")
    key(".")
    key("o")
    key_code(124)  # right
    key_code(125)  # down
    key_code(49)   # space
    screenshot("09-tile-paint-keyframe")
    key("t")

    key("r")
    time.sleep(3.5)
    screenshot("10-recording-active")
    key("r")
    time.sleep(0.4)
    screenshot("11-recording-stop-modal")
    key("3")  # Discard the draft; the runbook capture should not publish scores.


if __name__ == "__main__":
    main()
