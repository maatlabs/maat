pub fn split(s: str, delimiter: str) -> [str] {
    s.split(delimiter)
}

pub fn join(parts: [str], separator: str) -> str {
    parts.join(separator)
}

pub fn trim(s: str) -> str {
    s.trim()
}

pub fn contains(s: str, pattern: str) -> bool {
    s.contains(pattern)
}

pub fn starts_with(s: str, prefix: str) -> bool {
    s.starts_with(prefix)
}

pub fn ends_with(s: str, suffix: str) -> bool {
    s.ends_with(suffix)
}

pub fn parse_int(s: str) -> i64 {
    s.parse_int()
}
