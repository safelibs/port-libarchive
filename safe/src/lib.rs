pub mod common {
    pub(crate) mod api;
    pub mod error;
    pub(crate) mod helpers;
    pub mod panic_boundary;
    pub mod state;
}

pub mod entry;
pub mod ffi;
pub mod r#match;
pub mod util;

pub(crate) mod generated {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

pub use generated::{
    LIBARCHIVE_CMAKE_INTERFACE_VERSION, LIBARCHIVE_LIBTOOL_AGE, LIBARCHIVE_LIBTOOL_CURRENT,
    LIBARCHIVE_PACKAGE_VERSION, LIBARCHIVE_SONAME, LIBARCHIVE_VERSION_NUMBER,
    LIBARCHIVE_VERSION_STRING,
};
