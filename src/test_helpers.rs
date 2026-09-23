macro_rules! dt {
    ($datetime:expr) => {
        ::chrono::NaiveDateTime::parse_from_str($datetime, "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc()
    };
}

macro_rules! dt_ms {
    ($ms:expr) => {
        DateTime::<Utc>::from_timestamp_millis($ms).expect("incorrect millis value")
    };
}

macro_rules! candle {
    ($timestamp:expr, $open:expr, $low:expr, $high:expr, $close:expr, $volume:expr) => {
        Candle {
            timestamp: dt!($timestamp),
            open: ::rust_decimal_macros::dec!($open),
            low: ::rust_decimal_macros::dec!($low),
            high: ::rust_decimal_macros::dec!($high),
            close: ::rust_decimal_macros::dec!($close),
            volume: Some(::rust_decimal_macros::dec!($volume)),
        }
    };

    ($timestamp:expr, $open:expr, $low:expr, $high:expr, $close:expr) => {
        Candle {
            timestamp: dt!($timestamp),
            open: ::rust_decimal_macros::dec!($open),
            low: ::rust_decimal_macros::dec!($low),
            high: ::rust_decimal_macros::dec!($high),
            close: ::rust_decimal_macros::dec!($close),
            volume: None,
        }
    };
}
