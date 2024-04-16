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

## Installing EGI v2

TODO