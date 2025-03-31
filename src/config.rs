use std::{fmt::Display, path::{Path, PathBuf}, str::FromStr};

use clap::Args;
use serde::{de, Deserialize, Serialize};

use ggg_rs::{i2s::{I2SHeaderEdit, I2SInputModifcations}, opus::{self, constants::bruker::BrukerParValue}};
use crate::default_files;


#[derive(Debug, Serialize, Deserialize)]
pub struct CoreConfig {
    /// The email address used to access the Caltech FTP server
    pub ftp_email: String,

    /// The email address at which you want to receive messages
    /// from the priors automation system. It does not need to
    /// be the same as the FTP email.
    pub priors_request_email: String,
}


#[derive(Debug, thiserror::Error)]
pub enum CommonConfigError {
    #[error("Error converting value: {0}")]
    CannotConvert(String),
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("More information required in the configuration: {0}")]
    UserInputReq(String),
}

#[derive(Debug, Args, Deserialize)]
pub struct DailyCommonArgs {
    /// A path with a date placeholder where interferograms are stored.
    /// 
    /// This uses curly braces to indicate a placeholder. The current date
    /// and the site ID can be inserted, replacing instances of {DATE}
    /// and {SITE_ID}, respectively. A format can also be given after a colon
    /// for DATE, e.g. {DATE:%Y%j} would be replaced with the four
    /// digit year and three digit day of year. If no format is given,
    /// as in {DATE}, it defaults to YYYY-MM-DD format.
    /// 
    /// Two examples, assuming that we are processing 1 Apr 2024 with site ID "xx",
    /// "/data/{DATE}/igms" would resolve to "/data/2024-04-01/igms",
    /// while "/data/{SITE_ID}/{DATE:%Y}/{DATE:%m}/{DATE:%d}/igms" would 
    /// resolve to "/data/xx/2024/04/01/igms".
    #[clap(short='i', long)]
    pub igram_pattern: String,

    /// A path with a date placeholder where I2S should be set up to run (required).
    /// 
    /// These paths can substitute in value using the same sort of patterns
    /// as IGRAM_PATTERN.
    #[clap(short='o', long)]
    pub run_dir_pattern: String,

    /// A path with an optional date placeholder pointing to the coordinates JSON file (required).
    /// 
    /// These paths can substitute in values using the same sort of patterns
    /// as IGRAM_PATTERN.
    #[clap(short='c', long)]
    pub coord_file_pattern: String,

    /// A path with a date placeholder pointing to the meteorology JSON file (required).
    /// 
    /// These paths can substitute in values using the same sort of patterns
    /// as IGRAM_PATTERN.
    #[clap(short='m', long)]
    pub met_file_pattern: String,

    /// A glob pattern to append to IGRAM_PATTERN that should return all interferograms
    /// for a given date (required). The same placeholder patterns as allowed in 
    /// IGRAM_PATTERN can be included, e.g. "ifg_{DATE:%Y%m%d}*" would search for files
    /// starting with "ifg_20240401" for 1 Apr 2024.
    #[clap(short='g', long, default_value_t = String::from("*"))]
    pub igram_glob_pattern: String,

    /// Which detector configuration the EM27 data used (required)
    /// 
    /// Options are "single" (for a standard InGaAs detector only)
    /// and "dual" (for a standard InGaAs plus an extended InGaAs
    /// to cover the CO band).
    #[clap(short='d', long)]
    #[serde(default, deserialize_with = "deserialize_detector_set_opt")]
    pub detectors: Option<DetectorSet>,

    /// A file containing the top part of an I2S input file (i.e. 
    /// the header parameters) to use as a template (optional). Note that
    /// some parameters will always be overwritten to handle the file
    /// structure and detectors. If omitted, the recommended top will
    /// be used.
    #[clap(short='t', long)]
    pub top_file: Option<PathBuf>,

    /// If given, the UTC offset to insert in the I2S input file header (optional).
    /// The default is "0.0", which assumes your interferograms were
    /// collected by a computer with the time set to UTC. Negative values
    /// are permitted.
    #[clap(short='u', long, allow_negative_numbers = true)]
    pub utc_offset: Option<String>,
}

impl DailyCommonArgs {
    pub fn read_from_path<P: AsRef<Path>>(p: P) -> Result<Self, CommonConfigError> {
        let rdr = std::fs::File::open(p.as_ref())
            .map_err(|e| CommonConfigError::IoError(
                format!("could not open JSON file {}: {e}", p.as_ref().display())
            ))?;

        let value: DailyCommonArgs = serde_json::from_reader(rdr)
            .map_err(|e| CommonConfigError::IoError(
                format!("the JSON file {} is not correct: {e}", p.as_ref().display())
            ))?;
            
        Ok(value)
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorSet {
    Single,
    Dual,
    MidIR,
}

impl DetectorSet {
    /// Infer the detector set to use for a collection of interferograms.
    /// 
    /// This will check the header of each interferogram passed in and confirm
    /// that all interferograms headers indicate the same set of detectors.
    /// If not, or if the slice of paths passed in is empty, this will return
    /// an error.
    /// 
    /// # See also
    /// - [`DetectorSet::infer_from_header`] to determine detectors for a single interferogram.
    pub fn infer_from_multi_headers<P: AsRef<Path>>(interferograms: &[P]) -> Result<DetectorSet, CommonConfigError> {
        if interferograms.len() == 0 {
            return Err(CommonConfigError::IoError("No inteferograms given, so no detectors to infer".to_string()));
        }
    
        let detectors = DetectorSet::infer_from_header(interferograms[0].as_ref())?;
        for igm in interferograms[1..].iter() {
            let this_det = DetectorSet::infer_from_header(igm.as_ref())?;
            if this_det != detectors {
                let igm0 = interferograms[0].as_ref().display();
                let igm = igm.as_ref().display();
                return Err(CommonConfigError::UserInputReq(
                    format!("different detector sets found in the given list of interferograms:\n{igm0} = {detectors}\n{igm} = {this_det}\nEither specify the correct detector set to use, or ensure each interferogram group has the same detector set.")
                ))
            }
        }
    
        Ok(detectors)
    }

    /// Infer the detector set to use for a single interferogram
    /// 
    /// This will check the header of the given interferogram and determine detector
    /// set to use for it.
    /// 
    /// # See also
    /// [`DetectorSet::infer_from_multi_headers`] to determine a single detector set to use for
    /// many interferograms, and verify that they all contain the same detectors.
    pub fn infer_from_header(interferogram: &Path) -> Result<DetectorSet, CommonConfigError> {
        let header = opus::IgramHeader::read_full_igram_header(interferogram)
            .map_err(|e| CommonConfigError::IoError(
                format!("Error reading interferogram {}: {e}", interferogram.display())
            ))?;

        let instrument = header.get_value(
            opus::constants::bruker::BrukerBlockType::InstrumentStatus,
            "INS"
        ).map_err(|e| CommonConfigError::IoError(
            format!("Could not find instrument name in header of {}: {e}", interferogram.display())
        ))?;

        let instrument = if let BrukerParValue::String(instr) = instrument {
            log::debug!("INS parameter value in {} = {instr}", interferogram.file_name().map(|s| s.to_string_lossy()).unwrap_or_default());
            instr
        } else {
            log::debug!("INS parameter value in {} was not a string", interferogram.file_name().map(|s| s.to_string_lossy()).unwrap_or_default());
            ""
        };

        if instrument == "EM27/SUN MIR" {
            // Jacob noted in the original EGI that this configuration is the rarest,
            // so we just assume that such an instrument will match this instrument string
            return Ok(Self::MidIR);
        }

        // Most instruments probably just set the instrument value to "EM27/SUN", so we can't
        // distinguish ones with and without the dual detector from the instrument name.
        // Instead, check the number of data points in the second channel; if this is present and
        // not 0, then we *should* have an extended InGaAs detector
        // TODO: test on some of the early Caltech data with only one detector (/oco2-data/tccon/data/caltech_em27)
        // to ensure this is reading the right NPT parameter.
        let npt2_res = header.get_value(
            opus::constants::bruker::BrukerBlockType::IgramSecondaryStatus,
            "NPT"
        );
        let npt2 = match npt2_res {
            Ok(BrukerParValue::Integer(v)) => {
                log::debug!("NPT2 parameter value in {} = {v}", interferogram.file_name().map(|s| s.to_string_lossy()).unwrap_or_default());
                *v
            },
            Err(_) => {
                log::debug!("NPT2 parameter was not present in {}, using 0 to determine detectors", interferogram.file_name().map(|s| s.to_string_lossy()).unwrap_or_default());
                0
            },
            Ok(value) => return Err(CommonConfigError::IoError(
                format!("Unexpected type for NPT2 parameter in {}, expected integer, got {}", interferogram.display(), value.opus_type())
            )),
        };

        if npt2 == 0 {
            Ok(Self::Single)
        } else {
            Ok(Self::Dual)
        }
    }

    /// Return the modifications to make to the I2S input file top to correctly
    /// process this detector set.
    /// 
    /// Note that this does *NOT* include the flimit file, that is provided separately
    /// since we must ensure that the I2S parameter points to a valid path, and will
    /// normally want to write the flimit file during setup.
    pub fn get_changes(&self) -> I2SInputModifcations {
        let changes = match self {
            DetectorSet::Single => {
                vec![I2SHeaderEdit{parameter: 7, value: "2 2".to_string()}, 
                     I2SHeaderEdit{parameter: 11, value: "AA".to_string()},
                     I2SHeaderEdit{parameter: 12, value: "aa".to_string()}]
            },
            DetectorSet::Dual => {
                vec![I2SHeaderEdit{parameter: 7, value: "1 2".to_string()}, 
                     I2SHeaderEdit{parameter: 11, value: "DA".to_string()},
                     I2SHeaderEdit{parameter: 12, value: "da".to_string()}]
            },
            DetectorSet::MidIR => {
                vec![I2SHeaderEdit{parameter: 7, value: "2 2".to_string()}, 
                     I2SHeaderEdit{parameter: 11, value: "CC".to_string()},
                     I2SHeaderEdit{parameter: 12, value: "cc".to_string()}]
            }
        };

        I2SInputModifcations::from(changes)
    }

    /// Get the _contents_ of the flimit file to use for this detector set.
    /// 
    /// This will provide the contents of the flimit file as a string, which will
    /// normally be written out in the I2S run directory during setup.
    pub fn get_flimit(&self) -> &'static str {
        match self {
            DetectorSet::Single => default_files::FLIMIT_SINGLE,
            DetectorSet::Dual => default_files::FLIMIT_DUAL,
            DetectorSet::MidIR => default_files::FLIMIT_MIDIR,
        }
    }
}

impl Display for DetectorSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DetectorSet::Single => write!(f, "InGaAs"),
            DetectorSet::Dual => write!(f, "extended InGaAs"),
            DetectorSet::MidIR => write!(f, "mid-IR"),
        }
    }
}

impl FromStr for DetectorSet {
    type Err = CommonConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "s" | "single" => Ok(Self::Single),
            "d" | "dual" => Ok(Self::Dual),
            "m" | "midir" => Ok(Self::MidIR),
            _ => Err(CommonConfigError::CannotConvert(
                format!("'{s}' is not a valid detector set")
            ))
        }
    }
}

fn deserialize_detector_set<'de, D>(deserializer: D) -> Result<DetectorSet, D::Error>
where D: serde::Deserializer<'de>
{
    let s = String::deserialize(deserializer)?;
    match DetectorSet::from_str(&s) {
        Ok(dset) => Ok(dset),
        Err(e) => Err(de::Error::custom(format!("{e}"))),
    }
}

fn deserialize_detector_set_opt<'de, D>(deserializer: D) -> Result<Option<DetectorSet>, D::Error>
where D: serde::Deserializer<'de>
{
    let det_set = deserialize_detector_set(deserializer)?;
    Ok(Some(det_set))
}