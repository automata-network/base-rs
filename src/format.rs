use std::prelude::v1::*;

use eth_types::SU256;

pub fn debug<T: std::fmt::Debug>(d: T) -> String {
    format!("{:?}", d)
}

pub fn ether_sub(a: &SU256, b: &SU256) -> String {
    let mut sign = "";
    let detla = if a > b { a - b } else { b - a };
    if a < b {
        sign = "-";
    }
    return format!("{}{}", sign, parse_ether(&detla, 18));
}

pub fn parse_ether(val: &SU256, decimal: usize) -> String {
    let mut val = format!("{}", val);
    if val.len() <= decimal {
        val = "0.".to_owned() + &"0".repeat(decimal - val.len()) + &val;
    } else {
        val = val[..val.len() - decimal].to_owned() + "." + &val[val.len() - decimal..];
    }
    val = val.trim_end_matches('0').trim_end_matches('.').to_owned();
    val
}

pub fn summarize_str(s: &str, length: usize) -> String {
    let half_length = length / 2;
    if s.len() > length {
        format!("{}...{}", &s[..half_length], &s[s.len() - half_length..])
    } else {
        String::from(s)
    }
}

pub fn read_ether(val: String, decimal: usize) -> SU256 {
    let mut val = normalize_ether(val);
    let origin = val.clone();
    match val.find('.') {
        Some(dot) => {
            val = val.trim_end_matches('0').into();
            assert!(decimal + 1 >= (val.len() - dot));
            val += &"0".repeat(decimal + 1 - (val.len() - dot));
            val.remove(dot);
            val = val.trim_start_matches('0').into();
        }
        None => {
            val += &"0".repeat(decimal);
        }
    }
    let out = SU256::from(val.as_str());
    let sanity_check = parse_ether(&out, decimal);
    assert_eq!(sanity_check, origin);
    out
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
