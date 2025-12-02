# Installation

## Installing Rust

EGI v2 is written in [Rust](https://www.rust-lang.org/), so you will need a Rust toolchain installed to compile it.
To check if you do, run the command `cargo --help` in a terminal.
(`cargo` is Rust's build tool and package manager.)
You should see output like this:

```bash
$ cargo --help
Rust's package manager

Usage: cargo [+toolchain] [OPTIONS] [COMMAND]
       cargo [+toolchain] [OPTIONS] -Zscript <MANIFEST_RS> [ARGS]...
```

If so, you can skip the rest of this section.
If not, installing Rust is simple and does not require administrative privileges.
If you are working on a high performance cluster, double check that Rust/cargo aren't already available by loading a module.

To install Rust, use the `rustup` manager by following the instructions [on the rust-lang website](https://www.rust-lang.org/tools/install).

## Installing GGG

EGI is a wrapper around the GGG retrieval software, developed at JPL.
You will need GGG installed for EGI to work.
Full instructions to install are given [on the TCCON wiki](https://tccon-wiki.caltech.edu/Main/GGG2020ReleaseNotes).
The very brief version is:

1. Ensure you have a Fortran compiler and the [`conda` package manager](https://www.anaconda.com/download/) installed and available on your system.
   The `miniconda` Python installation provides a minimal Python install and `conda` manager, and is ideal for this purpose.
   `gfortran` is the default Fortran compiler for GGG; to use another compiler will require you to link the proper compiler script in GGG's `install` subdirectory.
2. Download the latest release from https://github.com/TCCON/GGG and untar it.
3. Set the `GGGPATH` and `gggpath` environmental variables for your shell to point to the GGG directory.
   These should go in your `~/.bashrc` or equivalent file.
   For example, if `/home/user/ggg` is the directory that you expanded from the release tarball (it should contain subdirectories such as `isotopologs` and `linelist`),
   and you use Bash, then add the following to `~/.bashrc`:

```bash
export GGGPATH=/home/user/ggg
export gggpath=/home/user/ggg
```

If your shell is ZSH, then the syntax is the same, but you would modify `~/.zshrc` instead.
You can check which shell you have by running the command `echo $SHELL` in a terminal.

## Installing GGG-RS

EGI-RS uses some developmental replacements for the GGG post processing programs that have more flexibility to handle
EM27/SUN-specific settings.
Because this is part of a repo still under development, installation is a bit annoying at present.
We will do our best to simplify this in the future.

For now, the following steps will install these programs:

1. Set the environmental variable `GGGRS_NCDIR` to point to the environment created as part of installing GGG.
   Depending on your GGG installation process, this might be at `$GGGPATH/install/.condaenv` or a named environment
   managed by conda. If you do not have a `$GGGPATH/install/.condaenv`, run `conda env list` and find the path for
   an environment named "ggg-tccon-default".
   See the [Installing GGG](#installing-ggg) section above for how to set environmental variables.
   After modifying your `~/.bashrc` or `~/.zshrc` file, run `source ~/.bashrc` or `source ~/.zshrc` to include the
   new environmental variable in your shell.
2. Change directory into your `$GGGPATH`, directory (`cd $GGGPATH`)
3. Clone the GGG-RS repo as `src-rs` (`git clone https://github.com/TCCON/ggg-rs.git src-rs`)
4. Change into the `src-rs` directory and run the `make` command.

Compilation may take a few minutes.
If all goes well, you should see a message similar to the following:

```
   Installed package `ggg-rs v0.1.0 (/home/you/ggg/src-rs)` (executables `add_nc_flags`, `apply_tccon_airmass_correction`, `bin2nc`, `change_ggg_files`, `collate_tccon_results`, `i2s_setup`, `list_spectra`, `plot_opus_spectra`, `query_output`, `strip_header`)
warning: be sure to add `/home/you/ggg/bin` to your PATH to be able to run the installed binaries
```

You can ignore the warning about adding a directory to your PATH; we will always be calling these programs with their full path, so adding the directory to your PATH (which it so you only need to type the program name to run it).
If you encounter trouble, see the [GGG-RS README](https://github.com/TCCON/ggg-rs) for suggestions.
To free up space, you can run the `cargo clean` command in `$GGGPATH/src-rs` to delete intermediate compilation files not needed any more.

## Installing EGI-RS

Once you have GGG-RS installed, installing EGI-RS is easy.
Simply run the following command from anywhere:

```bash
cargo install --git https://github.com/TCCON/egi-rs --root "$GGGPATH"
```

Similarly to installing GGG-RS, you should see a message similar to:

```
   Installed package `egi-rs v0.1.0 (https://github.com/TCCON/egi-rs#2d7a442c)` (executables `em27-catalogue`, `em27-gfit-prep`, `em27-i2s-prep`, `em27-init`)
warning: be sure to add `/home/you/ggg/bin` to your PATH to be able to run the installed binaries
```

Again, we can ignore that warning.
The final step is to run the `em27-init` program we just installed.
This will add some extra EM27/SUN-specific files to your `$GGGPATH`.
It will print out a summary; all steps should read "OK" like so:

```
Summary:
   OK    Make directory $GGGPATH/egi
   OK    Create 'egi_config.toml' file
   OK    Create 'em27.gnd' file
   OK    Create 'EXAMPLE_EM27_qc.dat' file
   OK    Create 'EXAMPLE_EM27_extra_filters.toml' file
   OK    Create 'corrections_airmass_postavg.em27.dat' file
   OK    Create 'corrections_insitu_postavg.em27.dat' file
   OK    Add em27.gnd entry to windows.men
   OK    Find program 'collate_tccon_results'
   OK    Find program 'apply_tccon_airmass_correction'
   OK    Find program 'apply_tccon_insitu_correction'
   OK    Find program 'add_nc_flags'

EGI initialization complete.
```

If not, or if you got a fatal error earlier in the run, correct the issue and try again.
