#!/usr/bin/env python3
"""Serve the web interface on a PC, with a simulated tester behind it.

    python3 web/build.py
    python3 web/dev.py [port]           # then open http://localhost:8080/

It answers the same requests as the Wi-Fi module (api/state, api/command,
api/wifi) and passes commands to `sim_console`, the firmware's console
running against its software model. `POST /api/memory` with a body of
good, edo, empty, cell or short changes the simulated memory.
"""
import json
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DIST = ROOT / "dist"
MIME = {".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8",
        ".js": "text/javascript; charset=utf-8", ".svg": "image/svg+xml", ".woff2": "font/woff2"}

sim = subprocess.Popen(["cargo", "run", "-q", "-p", "rt-core", "--features", "sim", "--example",
                        "sim_console"], cwd=ROOT.parent / "firmware", stdin=subprocess.PIPE,
                       stdout=subprocess.PIPE, text=True)
lock = threading.Lock()
state = {"busy": False, "link": True, "result": None,
         "wifi": {"mode": "sta", "ssid": "workshop", "ip": "127.0.0.1"}}


def ask(line: str):
    with lock:
        sim.stdin.write(line + "\n")
        sim.stdin.flush()
        return json.loads(sim.stdout.readline())


def work(cmd: str) -> None:
    try:
        state["result"] = ask("json " + cmd)
    finally:
        state["busy"] = False


class Handler(BaseHTTPRequestHandler):
    def reply(self, code, body, mime="application/json"):
        data = body if isinstance(body, bytes) else json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", mime)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == "/api/state":
            return self.reply(200, state)
        path = DIST / ("index.html" if self.path == "/" else self.path.lstrip("/").split("?")[0])
        if path.is_file() and DIST in path.resolve().parents:
            return self.reply(200, path.read_bytes(), MIME.get(path.suffix, "application/octet-stream"))
        self.reply(404, {"error": "not found"})

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0))).decode()
        if self.path == "/api/command":
            if state["busy"]:
                return self.reply(409, {"error": "busy"})
            state["busy"], state["result"] = True, None
            threading.Thread(target=work, args=(body.strip(),), daemon=True).start()
            return self.reply(202, {"accepted": True})
        if self.path == "/api/memory":
            return self.reply(200, ask("!" + body.strip()))
        if self.path == "/api/wifi":
            return self.reply(200, {"saved": True})
        self.reply(404, {"error": "not found"})

    def log_message(self, *_):
        pass


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    print(f"http://localhost:{port}/")
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
