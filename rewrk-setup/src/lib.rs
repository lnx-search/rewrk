#[macro_use]
extern crate tracing;

use std::fmt::Display;
use rhai::{Dynamic, EvalAltResult, ImmutableString};

mod builtins;


fn register_builtins(engine: &mut rhai::Engine) {
    engine
        .register_fn("log_info", log_info)
        .register_fn("log_warn", log_warn)
        .register_fn("log_error", log_error)
        .register_fn("fetch", fetch);
}


fn log_info(t: &Dynamic) {
    info!("{}", t)
}

fn log_warn(t: &Dynamic) {
    warn!("{}", t)
}

fn log_error(t: &Dynamic) {
    error!("{}", t)
}

fn fetch(method: &str, ) -> Result<(), Box<EvalAltResult>> {
    todo!()
}
