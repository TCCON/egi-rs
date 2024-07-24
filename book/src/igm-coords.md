# Location

EGI v2 prefers JSON files to specify the latitude, longitude, and altitude at which the EM27 was positioned.
Unlike version 1, which recommended using coordinate files stored in your `EGIPATH`, we recommend for version 2
that you place these JSON files alongside your interferograms.
That way, if you package the interferograms to send to another computing system or institution, the necessary
location data goes along with those interferograms.
We do still support the original coordinate file approach, see below for more on that.

For this tutorial, we'll assume that your data will be in a directory structure like this:

```text
{{#include dir-tree.txt}}
```

where `xx` is your site ID. As you can see, the input data is organized by date, and each date has a subfolder containing
the interferograms as well as two JSON files.
We'll create the `coords.json` files now.
These files can have different formats, but the one we will use here represents a single location.
Since we usually separate our data by day, this format works for all cases where the EM27 was in one location for a given day
(which should be the majority of cases).

These JSON files have the format:

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

For this tutorial, we'll assume the EM27 was in the same place for both dates, so we would enter this same information for both files.
If your EM27 is stationed quasi-permanently at one location, you could create one JSON file and symbolically link it to each daily directory.

We will see how these files are used in [Running I2S](./run-i2s.md).

## Coordinate file support

If you have coordinate files from EGI v1, you can reuse them by making your `coords.json` files like so:

```json
{
  "site_id": "xx"
}
```

Again, "xx" would be replaced with the site ID for these interferograms.
This tells EGI v2 to look for a coordinate file at `$EGIPATH/coordinates/xx_dlla.dat`.
The coordinate files have the format:

```text
2   6
Date     UTCTime   Latitude  Longitude  Alt_masl  Descrip_opt
20140601 01:30:00   35.1431  -116.1042      237    Zzyxx (testing)
20140613 17:34:00   34.1362  -118.1269      237    Caltech
20140613 17:34:32   34.1361  -118.1269      237    CaltechB
20140628 18:55:05   34.1362  -118.1269      237    Caltech
20140628 18:55:30   35.1431  -116.1042      237    Zzyxx (testing)
```

where:

- the first line gives the number of header lines (2) and the number of columns (6) - the number of columns is ignored by EGI v2,
- "Date" and "UTCTime" give the starting date/time for these coordinates.
  - "UTCTime" is optional, if omitted.... TODO
- "Latitude" and "Longitude" give the coordinates, with south and west represented as negative,
- "Alt_masl" is the meters above sea level for this measurement, and
- "Descrip_opt" is an optional, human-readable location description (not read by EGI)
