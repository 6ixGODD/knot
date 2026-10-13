import json
from pathlib import Path
import sys
from knot import Model

if len(sys.argv) != 4:
    raise SystemExit('Usage: smoke.py <absolute native library> <speaker.json> <speaker-events.json>')
with Model.from_file(sys.argv[2], library=sys.argv[1]) as model:
    result = model.initial()
    for event in json.loads(Path(sys.argv[3]).read_text(encoding='utf-8')):
        result = model.apply(result.state, event)
        if result.state['revision'] in (5, 6):
            print(result.snapshot['price']['totalMinor'])
    model.evaluate(result.state)
