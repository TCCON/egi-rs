# Running i2s

Now that all of the original data is prepared, it's time to set up and run I2S, which converts the interferograms into spectra.
We use the `em27-i2s-prep` program to set up run directories and scripts for I2S.
This has several subcommands which are useful in different cases.
For this example, we'll use the `daily-json` subcommand.
This allows us to set up to run each day's worth of interferograms in parallel, with the configuration we need for that included in (yet another) JSON file.

First we need to prepare the JSON file.
It needs to include a number of pieces of information:

- the directories where the interferograms are stored,
- a glob pattern to select the interferograms in each directory,
- the paths to the coordinate and meteorology JSON files, and
- where we want the run directories to be created.

Continuing on from the previous example, let's assume that we're processing data for the instrument with site ID "xx", and we have a directory structure like so:

```text
{{#include dir-tree.txt}}
```

We'll also assume that our interferograms include the date in their name,
that we want to create run directories like `/data/xx/spectra/20240401` to run I2S in,
and that our EM27 has the dual InGaAs detector that supports retrieving CO.
Let's create a JSON file named `demo.json` with the following contents:

```json
{
  "igram_pattern": "/data/{SITE_ID}/{DATE:%Y%m%d}/interferograms/",
  "igram_glob_pattern": "*{DATE:%Y%m%d}*",
  "coord_file_pattern": "/data/{SITE_ID}/{DATE:%Y%m%d}/coords.json",
  "met_file_pattern": "/data/{SITE_ID}/{DATE:%Y%m%d}/met_source.json",
  "run_dir_pattern": "/data/{SITE_ID}/spectra/{DATE:%Y%m%d}"
}
```

Notice that our values have some parts in curly braces, namely `{SITE_ID}` and `{DATE:%Y%m%d}`.
These are _placeholders_, which will have the actual value substituted in for each date processed.
`{SITE_ID}` will be replaced with our two-letter site ID, `xx`, when we pass it on the command line.
`{DATE}` will be replaced with each date that we want to process.
The `{DATE}` placeholders also include an extra part, the `:%Y%m%d`.
This specifies the format the date should have; "%Y" means the 4-digit year, "%m" the 2-digit month, and "%d" the 2-digit day.
We use the `chrono` crate for dates, so all the format specifiers listed [on their strftime page](https://docs.rs/chrono/latest/chrono/format/strftime/index.html) can be used.
Other characters can be included as well, e.g. `%Y.%m.%d` would print "2024.04.01" for 1 Apr 2024.
With no format, `{DATE}` defaults to `%Y-%m-%d` format.

Now we can run `em27-i2s-prep` to create our run directories.
Assuming we want to run the 1st, 2nd, and 3rd of Apr 2024, the command is:

```bash
$ em27-i2s-prep daily-json demo.json xx 2024-04-01 2024-04-03
```

Note that the start and end dates _must_ be given in the YYYY-MM-DD, a.k.a. %Y-%m-%d format for command line arguments.
This may take a minute or two to run (it inspects the headers of every interferogram, which adds up with a lot of them), but will create:

- three run directories: `20240401`, `20240402`, and `20240403` in `/data/xx/spectra`, and
- a `multii2s.sh` file in your current directory.

The `multii2s.sh` file is a script that will run each day's interferograms through I2S.
It can be run in serial with `bash multii2s.sh`, but if your system has the [`parallel` tool](https://doi.org/10.5281/zenodo.1146014), we can run the days in parallel with the command:

```bash
$ parallel -t --delay=1 -j4 < multii2s.sh
```

The `-j` argument specifies how many concurrent tasks to run, here we use 4, but you can use more or less (depending on your system).
Note that, if you are working over an SSH connection, it may be a good idea to run this command in something like [screen](https://www.gnu.org/software/screen/)
so that you can disconnect and let it keep running.
Depending on the number of days and interferograms per day, this step could take minutes or a few hours.
When it completes, you will have spectra in each of the run directories.

Now we're ready to run the level 2 retrieval.