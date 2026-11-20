pub const INK_COUNT: u8 = 10;

pub fn ink_for(key: &str) -> u8 {
    let mut hash: u32 = 0x811c_9dc5;
    for ch in key.to_lowercase().chars() {
        hash ^= ch as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash % u32::from(INK_COUNT)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_stable_and_case_insensitive() {
        assert_eq!(ink_for("maya@northwind.example"), ink_for("Maya@Northwind.Example"));
        assert!(ink_for("") < INK_COUNT);
    }

    #[test]
    fn matches_the_typescript_implementation() {
        for (key, expected) in [
            ("a", 0),
            ("maya@northwind.example", 0),
            ("dad@example.com", 5),
            ("田中@example.jp", 4),
            ("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", 5),
            ("Ünïcödé@Example.COM", 5),
        ] {
            assert_eq!(ink_for(key), expected, "ink for {key:?}");
        }
    }

    #[test]
    fn spreads_across_the_whole_palette() {
        let seen: std::collections::HashSet<u8> =
            (0..400).map(|i| ink_for(&format!("person{i}@example.com"))).collect();
        assert_eq!(seen.len(), usize::from(INK_COUNT));
    }
}
