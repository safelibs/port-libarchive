pub mod common {
    pub mod error;
    pub mod panic_boundary;
    pub mod state;
}

pub mod ffi;

pub(crate) mod generated {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

pub use generated::{
    LIBARCHIVE_CMAKE_INTERFACE_VERSION, LIBARCHIVE_LIBTOOL_AGE, LIBARCHIVE_LIBTOOL_CURRENT,
    LIBARCHIVE_PACKAGE_VERSION, LIBARCHIVE_SONAME, LIBARCHIVE_VERSION_NUMBER,
    LIBARCHIVE_VERSION_STRING,
};
