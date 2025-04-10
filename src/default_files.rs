use crate::config::CoreConfig;
pub use inner::*;

pub fn default_core_config_toml() -> String {
    let default_cfg = CoreConfig {
        ftp_email: "you@example.com".to_string(),
        priors_request_email: "you@example.com".to_string(),
    };
    let s = toml::to_string_pretty(&default_cfg)
        .expect("failed to serialize the default core configuration as TOML - this is a bug");
    s
}

#[cfg(unix)]
mod inner {
    pub static I2S_TOP: &'static str = include_str!("etc/em27_i2s.top");
    pub static FLIMIT_SINGLE: &'static str = include_str!("etc/flimit-dual.i2s");
    pub static FLIMIT_DUAL: &'static str = include_str!("etc/flimit-dual.i2s");
    pub static FLIMIT_MIDIR: &'static str = include_str!("etc/flimit-mid-ir.i2s");
    pub static EM27_WINDOWS: &'static str = include_str!("etc/em27_windows.gnd");
    pub static EM27_QC: &'static str = include_str!("etc/example_em27_qc.dat");
    pub static EM27_ADCFS: &'static str = include_str!("etc/corrections_airmass_postavg.em27.dat");
    pub static EM27_AICFS: &'static str = include_str!("etc/corrections_insitu_postavg.em27.dat");
    pub static POSTPROC_SCRIPT: &'static str = include_str!("etc/post_processing.sh");
}

#[cfg(windows)]
mod inner {
    pub static I2S_TOP: &'static str = include_str!(r"etc\em27_i2s.top");
    pub static FLIMIT_SINGLE: &'static str = include_str!(r"etc\flimit-dual.i2s");
    pub static FLIMIT_DUAL: &'static str = include_str!(r"etc\flimit-dual.i2s");
    pub static FLIMIT_MIDIR: &'static str = include_str!(r"etc\flimit-mid-ir.i2s");
    pub static EM27_WINDOWS: &'static str = include_str!(r"etc\em27_windows.gnd");
    pub static EM27_QC: &'static str = include_str!(r"etc\example_em27_qc.dat");
    pub static EM27_ADCFS: &'static str = include_str!(r"etc\corrections_airmass_postavg.em27.dat");
    pub static EM27_AICFS: &'static str = include_str!(r"etc\corrections_insitu_postavg.em27.dat");
    pub static POSTPROC_SCRIPT: &'static str = include_str!(r"etc\post_processing.sh");
}
