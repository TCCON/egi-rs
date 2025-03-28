pub use inner::*;

#[cfg(unix)]
mod inner {
    pub static I2S_TOP: &'static str = include_str!("etc/em27_i2s.top");
    pub static FLIMIT_SINGLE: &'static str = include_str!("etc/flimit-dual.i2s");
    pub static FLIMIT_DUAL: &'static str = include_str!("etc/flimit-dual.i2s");
    pub static FLIMIT_MIDIR: &'static str = include_str!("etc/flimit-mid-ir.i2s");
    pub static POSTPROC_SCRIPT: &'static str = include_str!("etc/post_processing.sh");
}

#[cfg(windows)]
mod inner {
    pub static I2S_TOP: &'static str = include_str!(r"etc\em27_i2s.top");
    pub static FLIMIT_SINGLE: &'static str = include_str!(r"etc\flimit-dual.i2s");
    pub static FLIMIT_DUAL: &'static str = include_str!(r"etc\flimit-dual.i2s");
    pub static FLIMIT_MIDIR: &'static str = include_str!(r"etc\flimit-mid-ir.i2s");
    pub static POSTPROC_SCRIPT: &'static str = include_str!(r"etc\post_processing.sh");
}

