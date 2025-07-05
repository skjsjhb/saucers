#![feature(vec_into_raw_parts)]
#![feature(mpmc_channel)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

pub mod app;
mod capi;
pub mod collector;
pub mod icon;
pub mod options;
pub mod prefs;
pub mod stash;
pub mod webview;
