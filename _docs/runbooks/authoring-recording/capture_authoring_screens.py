#!/usr/bin/env python3
"""Capture Arenic authoring screenshots through Chrome DevTools Protocol.

This intentionally uses only Python's standard library so the runbook can be
regenerated on a clean workstation without adding project dependencies.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import random
import socket
import struct
import time
import urllib.parse
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "_docs" / "runbooks" / "authoring-recording" / "screenshots"
CDP_LIST = "http://127.0.0.1:9222/json/list"
APP_URL = "127.0.0.1:8081"


class WebSocket:
    def __init__(self, url: str) -> None:
        parsed = urllib.parse.urlparse(url)
        self.host = parsed.hostname or "127.0.0.1"
        self.port = parsed.port or 80
        self.path = parsed.path
        if parsed.query:
            self.path += "?" + parsed.query
        self.sock = socket.create_connection((self.host, self.port), timeout=10)
        self._handshake()

    def _handshake(self) -> None:
        key = base64.b64encode(os.urandom(16)).decode("ascii")
        request = (
            f"GET {self.path} HTTP/1.1\r\n"
            f"Host: {self.host}:{self.port}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n"
            "\r\n"
        )
        self.sock.sendall(request.encode("ascii"))
        response = self.sock.recv(4096)
        if b" 101 " not in response.split(b"\r\n", 1)[0]:
            raise RuntimeError(f"WebSocket handshake failed: {response[:200]!r}")
        accept = base64.b64encode(
            hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()
        )
        if accept not in response:
            raise RuntimeError("WebSocket accept header did not match")

    def send_json(self, payload: dict) -> None:
        data = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        header = bytearray([0x81])
        if len(data) < 126:
            header.append(0x80 | len(data))
        elif len(data) < 65536:
            header.append(0x80 | 126)
            header.extend(struct.pack("!H", len(data)))
        else:
            header.append(0x80 | 127)
            header.extend(struct.pack("!Q", len(data)))
        mask = random.randbytes(4)
        masked = bytes(byte ^ mask[i % 4] for i, byte in enumerate(data))
        self.sock.sendall(bytes(header) + mask + masked)

    def recv_json(self) -> dict:
        chunks: list[bytes] = []
        while True:
            first_two = self._recv_exact(2)
            b1, b2 = first_two
            fin = bool(b1 & 0x80)
            opcode = b1 & 0x0F
            masked = bool(b2 & 0x80)
            length = b2 & 0x7F
            if length == 126:
                length = struct.unpack("!H", self._recv_exact(2))[0]
            elif length == 127:
                length = struct.unpack("!Q", self._recv_exact(8))[0]
            mask = self._recv_exact(4) if masked else b""
            payload = self._recv_exact(length) if length else b""
            if masked:
                payload = bytes(byte ^ mask[i % 4] for i, byte in enumerate(payload))
            if opcode == 0x8:
                raise RuntimeError("WebSocket closed")
            if opcode == 0x9:
                self._send_control(0xA, payload)
                continue
            if opcode in (0x1, 0x2, 0x0):
                chunks.append(payload)
                if fin:
                    return json.loads(b"".join(chunks).decode("utf-8"))

    def _send_control(self, opcode: int, payload: bytes) -> None:
        mask = random.randbytes(4)
        header = bytes([0x80 | opcode, 0x80 | len(payload)])
        masked = bytes(byte ^ mask[i % 4] for i, byte in enumerate(payload))
        self.sock.sendall(header + mask + masked)

    def _recv_exact(self, size: int) -> bytes:
        data = bytearray()
        while len(data) < size:
            chunk = self.sock.recv(size - len(data))
            if not chunk:
                raise RuntimeError("socket closed while reading")
            data.extend(chunk)
        return bytes(data)


class CDP:
    def __init__(self, ws_url: str) -> None:
        self.ws = WebSocket(ws_url)
        self.next_id = 1
        self.events: list[dict] = []

    def call(self, method: str, params: dict | None = None) -> dict:
        ident = self.next_id
        self.next_id += 1
        self.ws.send_json({"id": ident, "method": method, "params": params or {}})
        while True:
            msg = self.ws.recv_json()
            if msg.get("id") == ident:
                if "error" in msg:
                    raise RuntimeError(f"{method}: {msg['error']}")
                return msg.get("result", {})
            self.events.append(msg)


KEYS = {
    "Enter": ("Enter", "Enter", 13),
    "Escape": ("Escape", "Escape", 27),
    "Space": (" ", "Space", 32),
    "Tab": ("Tab", "Tab", 9),
    "ArrowUp": ("ArrowUp", "ArrowUp", 38),
    "ArrowDown": ("ArrowDown", "ArrowDown", 40),
    "ArrowLeft": ("ArrowLeft", "ArrowLeft", 37),
    "ArrowRight": ("ArrowRight", "ArrowRight", 39),
    "F1": ("F1", "F1", 112),
    "F5": ("F5", "F5", 116),
    ",": (",", "Comma", 188),
    ".": (".", "Period", 190),
    "/": ("/", "Slash", 191),
    "\\": ("\\", "Backslash", 220),
    "-": ("-", "Minus", 189),
    "=": ("=", "Equal", 187),
    "[": ("[", "BracketLeft", 219),
    "]": ("]", "BracketRight", 221),
}

for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ":
    KEYS[c] = (c.lower(), f"Key{c}", ord(c))
for c in "0123456789":
    KEYS[c] = (c, f"Digit{c}", ord(c))


def key(cdp: CDP, key_name: str, *, shift: bool = False, ctrl: bool = False, alt: bool = False) -> None:
    key_value, code, vk = KEYS[key_name]
    modifiers = (8 if shift else 0) | (2 if ctrl else 0) | (1 if alt else 0)
    text = ""
    if len(key_value) == 1 and not ctrl and not alt:
        text = key_value.upper() if shift else key_value
    for kind in ("rawKeyDown", "keyUp"):
        params = {
            "type": kind,
            "key": key_value.upper() if shift and len(key_value) == 1 else key_value,
            "code": code,
            "windowsVirtualKeyCode": vk,
            "nativeVirtualKeyCode": vk,
            "modifiers": modifiers,
        }
        if kind == "rawKeyDown" and text:
            params["text"] = text
            params["unmodifiedText"] = key_value
        cdp.call("Input.dispatchKeyEvent", params)
    time.sleep(0.15)


def click(cdp: CDP, x: int, y: int) -> None:
    cdp.call("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": x, "y": y})
    cdp.call("Input.dispatchMouseEvent", {"type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1})
    cdp.call("Input.dispatchMouseEvent", {"type": "mouseReleased", "x": x, "y": y, "button": "left", "clickCount": 1})
    time.sleep(0.3)


def screenshot(cdp: CDP, name: str) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    result = cdp.call("Page.captureScreenshot", {
        "format": "png",
        "fromSurface": True,
        "captureBeyondViewport": False,
    })
    (OUT / f"{name}.png").write_bytes(base64.b64decode(result["data"]))
    print(OUT / f"{name}.png")


def wait(seconds: float) -> None:
    time.sleep(seconds)


def page_ws_url() -> str:
    pages = json.loads(urllib.request.urlopen(CDP_LIST, timeout=5).read())
    for page in pages:
        if page.get("type") == "page" and APP_URL in page.get("url", ""):
            return page["webSocketDebuggerUrl"]
    raise RuntimeError(f"Could not find page containing {APP_URL}")


def main() -> None:
    cdp = CDP(page_ws_url())
    cdp.call("Page.enable")
    cdp.call("Runtime.enable")
    cdp.call("Log.enable")
    cdp.call("Page.navigate", {"url": "http://127.0.0.1:8081/"})
    cdp.call("Emulation.setDeviceMetricsOverride", {
        "width": 1280,
        "height": 720,
        "deviceScaleFactor": 1,
        "mobile": False,
    })
    cdp.call("Input.setIgnoreInputEvents", {"ignore": False})
    wait(3.0)
    screenshot(cdp, "01-title-screen")

    click(cdp, 640, 456)
    wait(5.0)
    screenshot(cdp, "02-overworld-author-mode")

    key(cdp, "P")
    wait(1.0)
    screenshot(cdp, "03-single-arena-author-hud")

    key(cdp, "F1")
    wait(0.5)
    screenshot(cdp, "04-dope-sheet-help-overlay")
    key(cdp, "F1")
    wait(0.3)

    key(cdp, "E")
    wait(0.5)
    screenshot(cdp, "05-entity-browser")
    key(cdp, "Escape")
    wait(0.3)

    key(cdp, "T")
    wait(0.5)
    screenshot(cdp, "06-tile-editor-open")
    key(cdp, "N")
    wait(0.3)
    key(cdp, ".")
    wait(0.2)
    key(cdp, "O")
    wait(0.2)
    key(cdp, "ArrowRight")
    key(cdp, "ArrowDown")
    key(cdp, "Space")
    wait(0.5)
    screenshot(cdp, "07-tile-paint-keyframe")
    key(cdp, "T")
    wait(0.3)

    key(cdp, "D")
    wait(0.3)
    screenshot(cdp, "08-heroic-difficulty")
    key(cdp, "D")
    wait(0.3)
    screenshot(cdp, "09-mythic-difficulty")


if __name__ == "__main__":
    main()
