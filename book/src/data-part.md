# Data partition

GGG uses a "data partition" file to determine which directories to search for spectra to process.
This is simply a text file with one directory per line, located at `$GGGPATH/config/data_part.lst`.
EGI-RS includes a program to help add directories to this file.

```admonish note
If you edit the `data_part.lst` file by hand, a command mistake in this file is to
forget to include a trailing path separator. That is, `/home/user/spectra/` is correct
to indicate that the spectra can be found in the `spectra` directory in the user's home
directory, but `/home/user/spectra` will cause `gsetup` to look in `/home/user` for spectra
with names _starting with_ "spectra".
```

The simplest way to list the directories that need to be added to this file is to use
the `em27-gfit-prep` command's `list-data-partitions-daily-json` subcommand.
For this example, let's assume that you have the JSON file `demo.json` we used in the
[run-i2s](./run-i2s.md) section and we want to run it for the same two days.
Now our command is:

```
$ em27-gfit-prep list-data-partitions-daily-json demo.json xx 2024-04-01 2024-04-03
```

This would print out two lines with the directories you need to add to the `data_part.lst` file:

```
/data/xx/spectra/20240401/spectra/
/data/xx/spectra/20240402/spectra/
```

This comes from the `run_dir_pattern` that we defined in our JSON file, which remember was `/data/{SITE_ID}/spectra/{DATE:%Y%m%d}`.
(The final "spectra" path component is always added, since that is built into the EGI-RS run directory structure for I2S.)
These are the exact lines you would add to `$GGGPATH/config/data_part.lst`.
You can either do this manually (by editing `data_part.lst` with a text editor and copying these into it), or
by using shell redirection to append these directly.
Assuming you are using a Unix-y shell like Bash or Zsh, that would look like this:

```
$ em27-gfit-prep list-data-partitions-daily-json demo.json xx 2024-04-01 2024-04-03 >> $GGGPATH/config/data_part.lst
```

Note that this does not do anything to check that these paths don't already exist in `data_part.lst`.
If they are duplicated, it should not hurt anything, but may lead to a rather long and messy `data_part.lst` file
(and may slow down the process of searching for spectra during sunrun/runlog creation and retrieval if it has
too many directories to search).