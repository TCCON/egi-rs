# Surface meteorology

Surface pressure is required to perform the level 2 retrievals, so it must be available when processing the EM27 interferograms.
Temperature, relative humidity, wind speed, and wind direction are convenient to have, but not required.
Since there is not a standard way of collecting surface meteorology data for EM27 observations, EGI must be flexible in how it accepts this data.

**Note:** users are _strongly_ discouraged from using surface pressure from reanalysis meteorology for their surface pressure unless _absolutely_ necessary.
Surface pressure is needed to accurately calculate the position of the EM27 relative to the atmosphere above it, so a good quality measured surface pressure
will give better results.

Like the location information, the source or sources of surface met data are configured via JSON files.
Currently, EGI assumes there will be one JSON file per day, but this will be more flexible in the future.

## Met formats

### JPL Vaisala V1

JPL uses Vaisala weather stations with data recorded by a simple Powershell script.
This outputs a UTF-16 encoded, space-separated file with the following format:

```text
YYYYMMDD,HH:MM,Data,Temperature,Humidity,Pressure
20230826,16:14,0R2,Ta=0.0#,Ua=0.0#,Pa=0.0#
20230826,16:15,0R2,Ta=26.8C,Ua=39.3P,Pa=972.7H
20230826,16:16,0R2,Ta=26.8C,Ua=40.3P,Pa=972.7H
...
```

EGI will discard the second line, which contains junk values.
Note that temperature is in degrees Celsius, humidity is in percent, and pressure is in hectopascals.
The JSON file used to tell EGI to parse this file has the format:

```json
{
  "type": "JplVaisalaV1",
  "file": DATA_FILE
}
```

Here, the value of "type" must be _exactly_ "JplVaisalaV1", and the value of "file" is the path pointing to the data file (shown above).
Relative paths will be interpreted as relative to the JSON file, so if the JSON file was:

```json
{
  "type": "JplVaisalaV1",
  "file": "./20230826_vaisala.txt"
}
```

then EGI would look for a file named `20230826_vaisala.txt` in the same directory as this JSON file.


### Caltech CSV V1

For EM27s operated by one of the Caltech-operated TCCON sites (TODO: list sites with .csv data available), met data from the TCCON site can be downloaded from TODO.
This site provides pressure, temperature, and humidity as separate `.csv` files, for example:

TODO: .csv files

To use these data as your met source, the JSON file would have the format:

```json
{
  "type": "CitCsvV1",
  "site": SITE_ID,
  "pres_file": PRESSURE_FILe,
  "temp_file": TEMPERATURE_FILE,
  "humid_file": HUMIDITY_FILE
}
```

The options are:

- "type" must have the _exact_ value "CitCsvV1",
- "site" must be the two-letter TCCON site ID for the site from which the met data were obtained: "ci" (Caltech), "df" (Armstrong/Dryden), "oc" (Lamont), or "pa" (Park Falls),
- "pres_file" must be the path to the pressure `.csv`,
- "temp_file" must be the path to the temperature `.csv`, and
- "humid_file" must be the path to the humidity `.csv`.

If relative, the "pres_file", "temp_file", and "humid_file" paths are interpreted as relative to the directory containing this JSON file.

A concrete example of a JSON file using Caltech met data is:

```json
{
  "type": "CitCsvV1",
  "site": "ci",
  "pres_file": "./2023-06-23-Pressure.csv",
  "temp_file": "./2023-06-23-Temp.csv",
  "humid_file": "./2023-06-23-Humidity.csv"
}
```