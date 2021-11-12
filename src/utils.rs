const GIGABYTE: f64 = (1024 * 1024 * 1024) as f64;
const MEGABYTE: f64 = (1024 * 1024) as f64;
const KILOBYTE: f64 = 1024_f64;

/// Dirt simple div mod function.
pub fn div_mod(main: u64, divider: u64) -> (u64, u64) {
    let whole = main / divider;
    let rem = main % divider;

    (whole, rem)
}

pub fn format_data(data_size: f64) -> String {
    if data_size > GIGABYTE as f64 {
        format!("{:.2} GB", data_size / GIGABYTE)
    } else if data_size > MEGABYTE as f64 {
        format!("{:.2} MB", data_size / MEGABYTE)
    } else if data_size > KILOBYTE as f64 {
        format!("{:.2} KB", data_size / KILOBYTE)
    } else {
        format!("{:.2} B", data_size)
    }
}
