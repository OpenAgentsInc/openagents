mod tables;

/// The number of words in `text`, skipping the stop words in the table.
pub fn count(text: &str) -> usize {
    text.split_whitespace()
        .filter(|word| !tables::STOP_WORDS.contains(word))
        .count()
}
