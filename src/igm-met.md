# Surface meteorology

Surface pressure is required to perform the level 2 retrievals, so it must be available when processing the EM27 interferograms.
Temperature, relative humidity, wind speed, and wind direction are convenient to have, but not required.
Since there is not a standard way of collecting surface meteorology data for EM27 observations, EGI must be flexible in how it accepts this data.

**Note:** users are discouraged from using surface pressure from reanalysis meteorology for their surface pressure unless _absolutely_ necessary.
Surface pressure is needed to accurately calculate the position of the EM27 relative to the atmosphere above it, so a good quality measured surface pressure
will give better results.

Continuing with our tutorial, recall the directory structure:

```text
{{#include dir-tree.txt}}
```

Now we are going to create the `met_source.json` files.

