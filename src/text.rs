pub fn truncate(
    text: &str,
    width: usize,
) -> &str {
    text.char_indices().nth(width).map_or(text, |(index, _)| &text[..index])
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncates_on_character_boundaries() {
        assert_eq!(truncate("µs-ok", 2), "µs");
        assert_eq!(truncate("short", 10), "short");
    }
}
