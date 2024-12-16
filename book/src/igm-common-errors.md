# Troublshooting interferogram conversion

This section describes common or difficult-to-understand errors that can occur while setting up to run I2S. 

## Errors during preparation

**What does the error "data did not match any variant of untagged enum CoordinateSource" mean when running em27-i2s-prep?**

This means that your [coordinate file](./igm-coords.md) did not match any of the expected formats.
That might mean you are missing one of the required fields (or misspelled one), or that a value is not of the proper type.
For instance, if any of "longitude", "latitude", or "altitude" are strings or `null`, that will cause this error.