use crate::{
    automata_runner::{AppendOnlySequence, ReadableView},
    dfa_earliest_pattern_matcher::DFAEarliestPatternMatcher,
};
use std::{collections::VecDeque, hash::Hash};

/// Given a stream, it masks the elements not appearing in any matching
/// pattern by replacing them with None. Elements that are part of a match
/// are preserved as Some(element).
pub struct MatchingFilter<'a, S, L> {
    /// The pattern matcher used to identify matches in the input stream
    matcher: DFAEarliestPatternMatcher<'a, S, L>,
    /// The input stream to filter
    input_stream: ReadableView<L>,
    /// Temporally queue
    temporally_queue: VecDeque<(L, bool)>,
    /// The output stream containing filtered elements (Some(L) for matched elements, None for unmatched)
    output_stream: AppendOnlySequence<Option<L>>,
}

impl<'a, S, L> MatchingFilter<'a, S, L>
where
    S: Eq + Hash + Clone,
    L: Eq + Hash + Clone,
{
    /// Creates a new MatchingFilter from a DFA and an input stream
    ///
    /// # Arguments
    ///
    /// * `matcher` - A DFAEarliestPatternMatcher that will be used to identify patterns
    /// * `input_stream` - The input stream to filter
    ///
    /// # Returns
    ///
    /// A new MatchingFilter instance
    pub fn new(
        matcher: DFAEarliestPatternMatcher<'a, S, L>,
        input_stream: ReadableView<L>,
    ) -> Self {
        Self {
            matcher,
            input_stream,
            temporally_queue: VecDeque::new(),
            output_stream: AppendOnlySequence::new(),
        }
    }

    /// Returns a ReadableView of the output stream
    ///
    /// # Returns
    ///
    /// A ReadableView of the filtered output stream
    pub fn readable_view(&self) -> ReadableView<Option<L>> {
        self.output_stream.readable_view()
    }

    /// Consumes elements from the input stream, processes them through the matcher,
    /// and updates the output stream accordingly.
    ///
    /// Elements that are part of a match are preserved as Some(element),
    /// while elements not part of any match are represented as None.
    pub fn consume_input(&mut self) {
        // The following is the invariant of the MatchingFilter:
        assert!(self.input_stream.start == self.temporally_queue.len() + self.output_stream.len());
        // Read elements from the input stream
        self.input_stream
            .readable_slice()
            .iter()
            .for_each(|element| {
                self.matcher.feed(element);
                self.temporally_queue.push_back((element.clone(), false));
                if let Some(matching_bound) = self.matcher.earliest_starting_position() {
                    // Move elements from the temporally_queue to the output_stream
                    for _i in self.output_stream.len()..matching_bound {
                        match self.temporally_queue.pop_front() {
                            Some((label, true)) => self.output_stream.append(Some(label)),
                            Some((_, false)) => self.output_stream.append(None),
                            None => panic!("Something is wrong with the temporally_queue"),
                        }
                    }
                    // Mark the elements in a match as matched
                    if let Some(i) = self.matcher.current_matching() {
                        let pos_in_queue = i - matching_bound;
                        for j in pos_in_queue..self.temporally_queue.len() {
                            self.temporally_queue[j].1 = true;
                        }
                    }
                } else {
                    // Move all the elements from the temporally_queue to the output_stream
                    for _i in 0..self.temporally_queue.len() {
                        match self.temporally_queue.pop_front() {
                            Some((label, true)) => self.output_stream.append(Some(label)),
                            Some((_, false)) => self.output_stream.append(None),
                            None => panic!("Something is wrong with the temporally_queue"),
                        }
                    }
                }
            });
        // process elements through the matcher
        let appended_length = self.input_stream.readable_slice().len();
        self.input_stream.advance_readable(appended_length);
        // All the elements in the input stream should be consumed
        assert_eq!(0, self.input_stream.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dfa::DFA;
    use std::collections::{HashMap, HashSet};

    // Define a simple enum for states in our test DFA
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum State {
        Initial,
        Middle,
        Final,
    }

    // Helper function to create a test DFA that matches the pattern "ab"
    fn create_test_dfa() -> DFA<State, char> {
        let states = vec![State::Initial, State::Middle, State::Final]
            .into_iter()
            .collect::<HashSet<_>>();

        let initial = State::Initial;
        let finals = vec![State::Final].into_iter().collect();

        let mut transitions = HashMap::new();
        transitions.insert((State::Initial, 'a'), State::Middle);
        transitions.insert((State::Middle, 'b'), State::Final);

        DFA {
            states,
            alphabet: vec!['a', 'b', 'c'].into_iter().collect(),
            initial,
            finals,
            transitions,
        }
    }

    #[test]
    fn test_matching_filter_basic() {
        // Create a DFA that matches "ab"
        let dfa = create_test_dfa();

        // Create a matcher from the DFA
        let matcher = DFAEarliestPatternMatcher::new(&dfa);

        // Create an input stream with "abc"
        let mut input_seq = AppendOnlySequence::new();
        input_seq.append('a');
        input_seq.append('b');
        input_seq.append('c');

        // Create a matching filter
        let mut filter = MatchingFilter::new(matcher, input_seq.readable_view());

        // Process the input
        filter.consume_input();

        // Check the output
        let output = filter.readable_view();
        let output_slice = output.readable_slice();

        // "a" and "b" should be preserved (Some), "c" should be filtered (None)
        assert_eq!(output_slice.len(), 3);
        assert_eq!(output_slice[0], Some('a'));
        assert_eq!(output_slice[1], Some('b'));
        assert_eq!(output_slice[2], None);
    }

    #[test]
    fn test_matching_filter_multiple_matches() {
        // Create a DFA that matches "ab"
        let dfa = create_test_dfa();

        // Create a matcher from the DFA
        let matcher = DFAEarliestPatternMatcher::new(&dfa);

        // Create an input stream with "abcabc"
        let mut input_seq = AppendOnlySequence::new();
        input_seq.append('a');
        input_seq.append('b');
        input_seq.append('c');
        input_seq.append('a');
        input_seq.append('b');
        input_seq.append('c');

        // Create a matching filter
        let mut filter = MatchingFilter::new(matcher, input_seq.readable_view());

        // Process the input
        filter.consume_input();

        // Check the output
        let output = filter.readable_view();
        let output_slice = output.readable_slice();

        // "a" and "b" should be preserved (Some), "c" should be filtered (None)
        assert_eq!(output_slice.len(), 6);
        assert_eq!(output_slice[0], Some('a'));
        assert_eq!(output_slice[1], Some('b'));
        assert_eq!(output_slice[2], None);
        assert_eq!(output_slice[3], Some('a'));
        assert_eq!(output_slice[4], Some('b'));
        assert_eq!(output_slice[5], None);
    }

    #[test]
    fn test_matching_filter_no_matches() {
        // Create a DFA that matches "ab"
        let dfa = create_test_dfa();

        // Create a matcher from the DFA
        let matcher = DFAEarliestPatternMatcher::new(&dfa);

        // Create an input stream with "ccc"
        let mut input_seq = AppendOnlySequence::new();
        input_seq.append('c');
        input_seq.append('c');
        input_seq.append('c');

        // Create a matching filter
        let mut filter = MatchingFilter::new(matcher, input_seq.readable_view());

        // Process the input
        filter.consume_input();

        // Check the output
        let output = filter.readable_view();
        let output_slice = output.readable_slice();

        // All elements should be filtered (None)
        assert_eq!(output_slice.len(), 3);
        assert_eq!(output_slice[0], None);
        assert_eq!(output_slice[1], None);
        assert_eq!(output_slice[2], None);
    }

    #[test]
    fn test_matching_filter_empty_input() {
        // Create a DFA that matches "ab"
        let dfa = create_test_dfa();

        // Create a matcher from the DFA
        let matcher = DFAEarliestPatternMatcher::new(&dfa);

        // Create an empty input stream
        let input_seq = AppendOnlySequence::<char>::new();

        // Create a matching filter
        let mut filter = MatchingFilter::new(matcher, input_seq.readable_view());

        // Process the input
        filter.consume_input();

        // Check the output
        let output = filter.readable_view();
        let output_slice = output.readable_slice();

        // Output should be empty
        assert_eq!(output_slice.len(), 0);
    }
}
