#[macro_use]
extern crate log;

#[cfg(target_os="android")]
#[allow(non_snake_case)]
pub mod android;

pub mod server;
pub mod client;
pub mod types;
