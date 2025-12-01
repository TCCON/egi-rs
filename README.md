# EGIRS (EM27/SUN GGG Interferogram processing suite in Rust)

A rewrite of the useful [EGI](https://tccon-wiki.caltech.edu/Main/EGI) scripts in Rust.

## Should I use this or the COCCON PROFFAST retrieval?

EM27/SUNs should first and foremost contribute to the [COCCON network](https://www.imkasf.kit.edu/english/COCCON.php), and follow their data processing standards. That includes using the PROFFAST retrieval and delivering data to the KIT archive. Use of EGI to process EM27/SUN data should be considered for secondary research purposes only.

## Status and roadmap

Currently only batch processing of interferograms to spectra is implemented.
Remaining steps include:

- Creation of sunruns and runlogs
- Downloading of `.mod` and `.vmr` files
- Retrieval directory setup
- Running GFIT
- Running post-processing (with EM27/SUN-specific corrections)
- Concatenation of multiple days' data

Some of these steps are blocked by work on other aspects of the GGG ecosystem, for instance, downloading the `.mod` and `.vmr` files in this version will use a JSON API under development for the GGG automatic priors system.

## Motivation

Routine processing of EM27/SUN data can be time consuming to do manually, hence the original EGI was developed to streamline the process.
For simplicity, it started as a collection of Bash and Matlab scripts, with Matlab eventually replaced with Python to remove the dependency on a paid Matlab license.
This became more complex over time as more use cases needed added, and making sure all the pieces work correctly together became more difficult.
Thus, I've decided to rewrite the original EGI into Rust to take advantage of Rust's strong type system to help make sure the pieces interoperate correctly.

Additionally, the current version of EGI is focused on end-to-end execution of GGG on EM27/SUN data.
This makes running tests where one part is changed to study the impact of different choices (e.g. priors, post-processing corrections) tricky.
Therefore, the intention is to make it easier to run individual components of EGIRS as well as do complete end-to-end runs.

## Usage

Use of EGIRS is documented in the associated [markdown book](https://tccon.github.io/egi-rs/).
