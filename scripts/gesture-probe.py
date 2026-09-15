#!/usr/bin/env python3
"""Drive `src/bin/gesture_probe.rs` over Rinch's debug IPC and print what fired.

Card C6's spike. The probe app writes every pointer event into one text node;
this connects to the debug server, synthesises presses and moves through the
*real* input path (`mouse_down` / `mouse_move` / `mouse_up` all go through
`RinchApp::handle_event`, exactly as a physical mouse does), then reads that
node back.

No coordinate is ever read off a picture: every one comes from a
`query_selector` layout box, which is what `docs/NOTES.md`'s HiDPI warning asks
for.

    cargo run --release --features devtools --bin gesture_probe &
    scripts/gesture-probe.py
"""

import json
import os
import socket
import struct
import sys
import time

APP = "gesture_probe"


def discover():
    """The port of the newest live gesture_probe, from ~/.rinch/debug."""
    d = os.path.join(os.path.expanduser("~"), ".rinch", "debug")
    best = None
    for name in os.listdir(d):
        try:
            entry = json.load(open(os.path.join(d, name)))
        except Exception:
            continue
        if entry.get("app_name") != APP:
            continue
        try:
            os.kill(entry["pid"], 0)
        except OSError:
            continue  # stale file from a dead run
        if best is None or entry["pid"] > best["pid"]:
            best = entry
    if best is None:
        sys.exit(f"no live {APP} found in {d} — is it running with --features devtools?")
    return best["port"]


class Client:
    def __init__(self, port):
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=10)
        self.id = 0
        self._frame(json.dumps({"protocol": "rinch-debug", "version": 1}).encode())
        hs = json.loads(self._read())
        assert hs.get("protocol") == "rinch-debug", hs

    def _frame(self, data):
        self.sock.sendall(struct.pack(">I", len(data)) + data)

    def _read(self):
        (n,) = struct.unpack(">I", self._recv_exact(4))
        return self._recv_exact(n)

    def _recv_exact(self, n):
        buf = b""
        while len(buf) < n:
            chunk = self.sock.recv(n - len(buf))
            if not chunk:
                raise EOFError("debug server closed the connection")
            buf += chunk
        return buf

    def cmd(self, method, **params):
        self.id += 1
        # `wait_frame` and friends are unit variants: serde rejects an empty
        # params map where it expects no field at all.
        req = {"id": self.id, "method": method}
        if params:
            req["params"] = params
        self._frame(json.dumps(req).encode())
        resp = json.loads(self._read())
        if resp.get("type") == "error":
            raise RuntimeError(f"{method}: {resp['message']}")
        return resp.get("data")

    # ── conveniences ────────────────────────────────────────────────────

    def select(self, selector):
        return self.cmd("query_selector", selector=selector)

    def one(self, selector):
        got = self.select(selector)
        if not got:
            sys.exit(f"nothing matched {selector}")
        return got[0]

    def box(self, node):
        a = node["absolute"]
        return a["x"], a["y"], a["width"], a["height"]

    def centre(self, node):
        x, y, w, h = self.box(node)
        return x + w / 2, y + h / 2

    def log(self):
        return self.one(".probe-log")["text_content"]

    def reset(self):
        x, y = self.centre(self.one(".probe-reset"))
        self.cmd("click", x=x, y=y)
        self.cmd("wait_frame")

    def drag(self, x0, y0, x1, y1, steps=8, settle=0.02):
        """A press, `steps` moves, a release — the way a hand does it."""
        self.cmd("mouse_move", x=x0, y=y0)
        self.cmd("mouse_down", x=x0, y=y0)
        for i in range(1, steps + 1):
            t = i / steps
            self.cmd("mouse_move", x=x0 + (x1 - x0) * t, y=y0 + (y1 - y0) * t)
            time.sleep(settle)
        self.cmd("mouse_up", x=x1, y=y1)
        self.cmd("wait_frame")


def banner(title):
    print(f"\n\033[1m── {title} " + "─" * max(0, 60 - len(title)) + "\033[0m")


def main():
    c = Client(discover())
    rows = c.select(".probe-row")
    handles = c.select(".probe-handle")
    lst = c.one(".probe-list")
    print(f"list box        {c.box(lst)}")
    print(f"{len(rows)} rows, first at {c.box(rows[0])}, second at {c.box(rows[1])}")

    # ── 1. Drag the handle of row 1 down over row 3 ─────────────────────
    banner("1. drag the handle downward, three rows")
    c.reset()
    hx, hy = c.centre(handles[1])
    _, ty = c.centre(rows[4])
    c.drag(hx, hy, hx, ty)
    print(c.log())

    # ── 2. Horizontal swipe on a row body (no handle) ───────────────────
    banner("2. swipe left across a row body, 120px")
    c.reset()
    rx, ry, rw, rh = c.box(rows[6])
    start_x = rx + rw - 20
    c.drag(start_x, ry + rh / 2, start_x - 120, ry + rh / 2)
    print(c.log())

    # ── 3. Vertical drag on a row body — the gesture a finger would use
    #       to scroll the list. On the desktop this is not scrolling at
    #       all; the point is what the row's own handlers see.
    banner("3. vertical drag across a row body, 200px")
    c.reset()
    rx, ry, rw, rh = c.box(rows[6])
    c.drag(rx + rw / 2, ry + rh / 2, rx + rw / 2, ry + rh / 2 - 200)
    print(c.log())

    # ── 4. Does the wheel — the only thing that scrolls this box on the
    #       desktop, and what Android turns a finger-drag into — reach the
    #       row, and does the list actually move? `delta_y` is added to the
    #       offset, so scrolling *down* is positive.
    banner("4. wheel over a row")
    c.reset()
    rx, ry, rw, rh = c.box(rows[6])
    c.cmd("scroll", x=rx + rw / 2, y=ry + rh / 2, delta_x=0.0, delta_y=180.0)
    c.cmd("wait_frame")
    print(c.log())
    print("row 0 box after scroll:", c.box(c.select(".probe-row")[0]))

    # ── 4b. …and does a drag still land on the right row once the list has
    #        been scrolled? Layout boxes are re-read after the scroll.
    banner("4b. drag a handle after scrolling")
    c.reset()
    rows2 = c.select(".probe-row")
    handles2 = c.select(".probe-handle")
    hx, hy = c.centre(handles2[6])
    _, ty = c.centre(rows2[9])
    print(f"handle 6 at {hx:.0f},{hy:.0f}; row 9 centre y {ty:.0f}")
    c.drag(hx, hy, hx, ty)
    print(c.log())
    c.cmd("scroll", x=rx + rw / 2, y=ry + rh / 2, delta_x=0.0, delta_y=-1000.0)
    c.cmd("wait_frame")

    # ── 5. A plain tap must still be a tap once handles are draggable ───
    banner("5. tap a row body")
    c.reset()
    x, y = c.centre(rows[2])
    c.cmd("click", x=x, y=y)
    c.cmd("wait_frame")
    print(c.log())

    # ── 6. A plain tap on the handle itself ─────────────────────────────
    banner("6. tap the drag handle")
    c.reset()
    x, y = c.centre(handles[2])
    c.cmd("click", x=x, y=y)
    c.cmd("wait_frame")
    print(c.log())


if __name__ == "__main__":
    main()
