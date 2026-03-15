// Creates a new empty set.
pub fn new_set() {
    Set::new()
}

// Returns a new set with the given value inserted.
pub fn set_insert(s: Set, value: i64) {
    s.insert(value)
}

// Returns true if the set contains the given value.
pub fn set_contains(s: Set, value: i64) -> bool {
    s.contains(value)
}

// Returns a new set with the given value removed.
pub fn set_remove(s: Set, value: i64) {
    s.remove(value)
}

// Returns the number of elements in the set.
pub fn set_len(s: Set) -> usize {
    s.len()
}

// Converts a set to an array of its elements.
pub fn set_to_array(s: Set) -> [i64] {
    s.to_array()
}
