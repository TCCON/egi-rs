use std::{path::PathBuf, process::ExitCode};

use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use error_stack::ResultExt;
use ggg_rs::i2s;
use egi_rs::i2s_catalog::{make_catalogue_entries, MainCatalogError};


fn main() -> ExitCode {
    let clargs = Cli::parse();

    env_logger::Builder::new()
    .filter_level(clargs.verbose.log_level_filter())
    .init();

    let res = driver(clargs);

    if let Err(e) = res {
        eprintln!("Error generating I2S catalog:\n{e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn driver(clargs: Cli) -> error_stack::Result<(), MainCatalogError> {
    let catalogue_entries = make_catalogue_entries(
        &clargs.coordinate_file,
        &clargs.surface_met_source_file,
        &clargs.interferograms,
        clargs.keep_if_missing_met
    )?;

    let mut stdout = std::io::stdout();
    i2s::write_opus_catalogue_table(&mut stdout, &catalogue_entries, false)
        .change_context_lazy(|| MainCatalogError::Catalog)?;
    Ok(())
}


/// Generate an I2S catalogue for EM27 interferograms
#[derive(Debug, clap::Parser)]
struct Cli {
    #[command(flatten)]
    verbose: Verbosity<WarnLevel>,
    
    /// Set this flag to include an interferogram even if there isn't surface met data available to match up with it.
    /// The default is to skip it, since GGG requires surface pressure to perform the retrieval.
    #[clap(long)]
    keep_if_missing_met: bool,

    /// Path to a coordinates JSON file (required). See the documentation for [`CoordinateSource`] for allowed formats.
    #[clap(long="coords")]
    coordinate_file: PathBuf,

    /// Path to a surface met source description file (required). See the documentation for [`MetSource`] for allowed formats.
    #[clap(long="surf-met",)]
    surface_met_source_file: PathBuf,

    /// Paths to the interferograms to add to the catalogue.
    interferograms: Vec<PathBuf>
}
