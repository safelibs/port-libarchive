#![allow(warnings, clippy::all)]

#[path = "support/mod.rs"]
mod support;

fn run_case(define_test: &str) {
    support::ported::run_ported_case("libarchive", define_test);
}

macro_rules! define_ported_tests {
    ($($define_test:ident),+ $(,)?) => {
        $(
            #[test]
            fn $define_test() {
                run_case(stringify!($define_test));
            }
        )+
    };
}

include!("libarchive/ported_cases.rs");
