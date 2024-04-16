# Location

EGI v2 uses JSON files to specify the latitude, longitude, and altitude at which the EM27 was positioned.
Version 2 switched to JSON files from the custom format used in version 1 to move towards standardized input file formats where possible.
The simplest (and currently only) format for these files represents a single location, for an EM27 which is deployed quasi-permanently.
These files have the format:

```json
{
  "longitude": LONGITUDE_DEG,
  "latitude": LATITUDE_DEG,
  "altitude": ALTITUDE_METERS
}
```

where `LONGITUDE_DEG`, `LATITUDE_DEG`, and `ALTITUDE_METERS` must be replaced with numeric values.
EGI uses the convention that west and south are represented as negative values.
Here is a concrete example for an EM27 operated at Caltech, which is at 118.13 W, 34.14 N, and 230 m altitude:

```json

{
  "longitude": -118.13,
  "latitude": 34.14,
  "altitude": 230.0
}
```

We will see how these files are used in [Running I2S](./run-i2s.md).
