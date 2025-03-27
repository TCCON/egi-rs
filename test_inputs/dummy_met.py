#!/usr/bin/env python3
from datetime import datetime, timezone
import json

fmt = '%Y-%m-%dT%H:%M:%S%z'
t1 = datetime(2025, 3, 1, 12, tzinfo=timezone.utc).strftime(fmt)
t2 = datetime(2025, 3, 1, 15, tzinfo=timezone.utc).strftime(fmt)
t3 = datetime(2025, 3, 1, 18, tzinfo=timezone.utc).strftime(fmt)
t4 = datetime(2025, 3, 1, 21, tzinfo=timezone.utc).strftime(fmt)

print(json.dumps({'datetime': t1, 'pressure': 1013.25}))
print(json.dumps({'datetime': t2, 'pressure': 1013.25, 'temperature': 25.0}))
print(json.dumps({'datetime': t3, 'pressure': 1013.25, 'humidity': 50.0}))
print(json.dumps({'datetime': t4, 'pressure': 1013.25, 'temperature': -10.0, 'humidity': 0.0}))
