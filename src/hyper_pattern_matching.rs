// Trait of pattern matching algorithms
trait HyperPatternMatching {
    // Feed a string-valued action to the given track
    fn feed(&mut self, action: &str, track: u32);
}
