#[macro_export]
macro_rules! debug {
    ( $arg:expr $( , $extra:expr )* ) => {
        #[cfg(feature = "telemetry")]
        tracing::debug!($arg $( , $extra )*);
    };
}

pub use debug;

#[macro_export]
macro_rules! error {
    ( $arg:expr $( , $extra:expr )* ) => {
        #[cfg(feature = "telemetry")]
        tracing::error!($arg $( , $extra )*);
    };
}

pub use error;
