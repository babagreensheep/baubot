//! Bindings to environment secrets for use across the workspace. Because accessing env secrets
//! sucks donkey balls.

pub use log::{debug, error, info, trace, warn};

include!(concat!(env!("OUT_DIR"), "/secrets.rs"));

/// Call this function to initialize loggers etc.. Env secrets are consts.
pub fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        // Initialise logger
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Trace)
            .filter_module("teloxide", log::LevelFilter::Off)
            .filter_module("reqwest", log::LevelFilter::Off)
            .parse_env("LOG_LEVEL")
            .init();

        log::info!("Init complete.");
    });
}
