pub fn trim(s: &mut String) {
    // Rust's trim() doesn't modify the string in-place, so we need to reassign
    let trimmed = s.trim().to_string();
    *s = trimmed;
}

pub fn trim_left(s: &mut String) {
    let trimmed = s.trim_start().to_string();
    *s = trimmed;
}

pub fn trim_right(s: &mut String) {
    let trimmed = s.trim_end().to_string();
    *s = trimmed;
}
