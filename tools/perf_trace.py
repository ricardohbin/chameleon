"""Summarize a Bevy chrome trace: which systems are slow.

    cargo run --features bevy/trace_chrome        # play, reproduce the lag, quit (writes trace-<time>.json)
    python tools/perf_trace.py trace-*.json [min_ms]

Prints the systems whose slowest run is over `min_ms` (default 20), sorted by worst run, with
how many times they were that slow, the total and the average time.
"""
import json
import sys
from collections import defaultdict

path = sys.argv[1]
min_ms = float(sys.argv[2]) if len(sys.argv) > 2 else 20.0
with open(path, encoding="utf-8") as f:
    text = f.read().rstrip().rstrip(",")
    if not text.endswith("]"):
        text += "]"  # a trace cut short by a crash/kill
    events = json.loads(text)

stacks = defaultdict(list)  # (pid, tid) -> [(name, ts)]
runs = defaultdict(list)  # name -> [ms]
for e in events:
    ph = e.get("ph")
    if ph == "X":
        runs[e["name"]].append(e.get("dur", 0) / 1000.0)
    elif ph == "B":
        stacks[(e.get("pid"), e.get("tid"))].append((e["name"], e["ts"]))
    elif ph == "E":
        st = stacks[(e.get("pid"), e.get("tid"))]
        if st:
            name, ts = st.pop()
            runs[name].append((e["ts"] - ts) / 1000.0)

rows = []
for name, ms in runs.items():
    worst = max(ms)
    if worst >= min_ms:
        rows.append((worst, name, sum(1 for v in ms if v >= min_ms), len(ms), sum(ms)))
rows.sort(reverse=True)
print(f"{'worst ms':>9} {'slow runs':>9} {'runs':>7} {'total ms':>10}  span")
for worst, name, slow, n, total in rows[:40]:
    print(f"{worst:9.1f} {slow:9d} {n:7d} {total:10.0f}  {name}")
