macro_rules! datetime {
    ($datetime:expr) => {
        ::chrono::NaiveDateTime::parse_from_str($datetime, "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc()
    };
}

macro_rules! candle {
    ($timestamp:expr, $open:expr, $low:expr, $high:expr, $close:expr, $volume:expr) => {
        Candle {
            timestamp: datetime!($timestamp),
            open: ::rust_decimal_macros::dec!($open),
            low: ::rust_decimal_macros::dec!($low),
            high: ::rust_decimal_macros::dec!($high),
            close: ::rust_decimal_macros::dec!($close),
            volume: Some(::rust_decimal_macros::dec!($volume)),
        }
    };

    ($timestamp:expr, $open:expr, $low:expr, $high:expr, $close:expr) => {
        Candle {
            timestamp: datetime!($timestamp),
            open: ::rust_decimal_macros::dec!($open),
            low: ::rust_decimal_macros::dec!($low),
            high: ::rust_decimal_macros::dec!($high),
            close: ::rust_decimal_macros::dec!($close),
            volume: None,
        }
    };
}
