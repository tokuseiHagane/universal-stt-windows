#!/usr/bin/env python3
"""Static file server with HTTP Range support for the wasm demo."""
import os
import re
import sys
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer

ROOT = sys.argv[1]
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 4173
RANGE = re.compile(r"bytes=(\d*)-(\d*)")


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=ROOT, **kwargs)

    def send_head(self):
        path = self.translate_path(self.path)
        if not os.path.isfile(path):
            return super().send_head()
        size = os.path.getsize(path)
        header = self.headers.get("Range")
        start, end = 0, size - 1
        status = 200
        if header:
            match = RANGE.search(header)
            if match:
                first, last = match.groups()
                if first:
                    start = int(first)
                    end = int(last) if last else size - 1
                elif last:
                    start = max(0, size - int(last))
                if start <= end and start < size:
                    end = min(end, size - 1)
                    status = 206
        handle = open(path, "rb")
        handle.seek(start)
        self.send_response(status)
        self.send_header("Content-Type", self.guess_type(path))
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Length", str(end - start + 1))
        if status == 206:
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.end_headers()
        return _Limited(handle, end - start + 1)


class _Limited:
    def __init__(self, handle, remaining):
        self.handle = handle
        self.remaining = remaining

    def read(self, amount=-1):
        if self.remaining <= 0:
            return b""
        if amount < 0 or amount > self.remaining:
            amount = self.remaining
        data = self.handle.read(amount)
        self.remaining -= len(data)
        return data

    def close(self):
        self.handle.close()


ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
