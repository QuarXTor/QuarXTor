use chrono::Local;

/// Formats the current system local time as `[YYYY-MM-DD HH:MM:SS.mmm]`.
pub fn ts_local() -> String {
    Local::now().format("[%Y-%m-%d %H:%M:%S%.3f]").to_string()
}

/// Prints a scoped log line with a consistent timestamp prefix.
pub fn log_line(scope: &str, level: &str, msg: &str) {
    println!("{} [{}] [{}] {}", ts_local(), scope, level, msg);
}

#[macro_export]
macro_rules! qlog_info {
    ($scope:expr, $($arg:tt)+) => {{
        $crate::util::log::log_line($scope, "INFO", &format!($($arg)+));
    }};
}

#[macro_export]
macro_rules! qlog_warn {
    ($scope:expr, $($arg:tt)+) => {{
        $crate::util::log::log_line($scope, "WARN", &format!($($arg)+));
    }};
}

#[macro_export]
macro_rules! qlog_err {
    ($scope:expr, $($arg:tt)+) => {{
        $crate::util::log::log_line($scope, "ERROR", &format!($($arg)+));
    }};
}

#[cfg(test)]
mod tests {
    use super::ts_local;

    #[test]
    fn format_shape_is_correct() {
        let ts = ts_local();
        assert!(ts.starts_with('['));
        assert!(ts.ends_with(']'));
        assert!(ts.len() >= "[1970-01-01 00:00:00.000]".len());
    }
}
