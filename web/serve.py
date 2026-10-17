#!/usr/bin/env python3
"""Serves web/dist like itch.io does: `.br` files with `Content-Encoding: br` and the type of the name without it.

    python web/serve.py [port] [folder]      # default: 8000, web/dist; then open http://localhost:8000
"""
import functools
import http.server
import os
import sys


class Handler(http.server.SimpleHTTPRequestHandler):
    def guess_type(self, path):
        path = str(path)
        if path.endswith((".br", ".gz")):
            path = path.rsplit(".", 1)[0]
        if path.endswith(".wasm"):
            return "application/wasm"
        return super().guess_type(path)

    def end_headers(self):
        name = self.path.split("?")[0]
        if name.endswith(".br"):
            self.send_header("Content-Encoding", "br")
        elif name.endswith(".gz"):
            self.send_header("Content-Encoding", "gzip")
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 else os.path.join(here, "dist")
    handler = functools.partial(Handler, directory=root)
    print(f"serving {root} on http://localhost:{port}")
    http.server.ThreadingHTTPServer(("", port), handler).serve_forever()
