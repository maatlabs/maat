pub fn new_set() {
    Set::new()
}

pub fn set_insert(s: Set, value: i64) {
    s.insert(value)
}

pub fn set_contains(s: Set, value: i64) -> bool {
    s.contains(value)
}

pub fn set_remove(s: Set, value: i64) {
    s.remove(value)
}

pub fn set_len(s: Set) -> usize {
    s.len()
}

pub fn set_to_array(s: Set) -> [i64] {
    s.to_array()
}
