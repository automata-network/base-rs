pub fn debug<T: std::fmt::Debug>(d: T) -> String {
    format!("{:?}", d)
}

pub fn summarize_str(s: &str, length: usize) -> String {
    let half_length = length / 2;
    if s.len() > length {
        format!("{}...{}", &s[..half_length], &s[s.len() - half_length..])
    } else {
        String::from(s)
    }
}


pub fn normalize_ether(origin: String) -> String {
    match origin.find('.') {
        Some(_) => {
            let mut origin = origin.trim_start_matches("0").to_owned();
            if origin.starts_with(".") {
                origin = "0".to_owned() + &origin;
            }
            origin
                .trim_end_matches("0")
                .trim_end_matches(".")
                .to_owned()
        }
        None => {
            let mut origin = origin.trim_start_matches("0").to_owned();
            if origin == "" {
                origin = "0".to_owned();
            }
            origin
        }
    }
}

pub fn truncate_ether(n: String, max: usize) -> String {
    match n.find('.') {
        None => return n,
        Some(idx) => {
            if n.len() - idx > max {
                return n[..idx + max].to_owned();
            }
            return n;
        }
    }
}

pub fn ternary<T>(n: bool, a: T, b: T) -> T {
    if n {
        a
    } else {
        b
    }
}
