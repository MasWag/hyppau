use crate::hyper_pattern_matching::HyperPatternMatching;
use crate::multi_stream_reader::MultiStreamReader;
use log::debug;

/// A scheduler that continuously reads from multiple input streams and feeds lines into a
/// [`HyperPatternMatching`] implementation.
///
/// # Type Parameters
///
/// * `Matching` - A type that implements the [`HyperPatternMatching`] trait.
///   This specifies the algorithm for hyper pattern matching.
pub struct ReadingScheduler<Matching: HyperPatternMatching> {
    matching: Matching,
    reader: MultiStreamReader,
}

impl<Matching: HyperPatternMatching> ReadingScheduler<Matching> {
    /// Creates a new `ReadingScheduler` from the given matching engine and `MultiStreamReader`.
    ///
    /// # Parameters
    ///
    /// * `matching` - An implementation of [`HyperPatternMatching`].
    /// * `reader` - A [`MultiStreamReader`] that manages multiple input streams.
    ///
    /// # Returns
    ///
    /// A new `ReadingScheduler` instance.
    pub fn new(matching: Matching, reader: MultiStreamReader) -> Self {
        Self { matching, reader }
    }

    /// Runs the scheduler until the end of all streams.
    ///
    /// The scheduler repeatedly reads lines from each available stream. When a line is
    /// successfully read, it is passed to the [`HyperPatternMatching::feed`] method, which
    /// processes it according to the pattern-matching logic.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Pseudocode snippet showing how you might run the scheduler:
    /// let matching = MyPatternMatcher::new(...);
    /// let reader = MultiStreamReader::new(...);
    /// let mut scheduler = ReadingScheduler::new(matching, reader);
    /// scheduler.run();
    /// ```
    pub fn run(&mut self) {
        let mut done: Vec<bool> = (0..self.reader.size()).map(|_| false).collect();
        while done.iter().any(|x| !*x) {
            for i in 0..self.reader.size() {
                if !done[i] {
                    let line = self.reader.read_line(i);
                    if line.is_err() {
                        done[i] = true;
                    } else {
                        let line = line.unwrap().trim_end().to_string();
                        self.matching.feed(&line, i);
                        let availability = self.reader.is_available(i);
                        done[i] = availability.is_err() || availability.is_ok_and(|f| !f);
                    }
                    if done[i] {
                        debug!("stream {} is closed", i);
                        self.matching.set_eof(i);
                    }
                }
            }
        }

        self.matching.consume_remaining();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::Automata;
    use crate::automata_runner::AppendOnlySequence;
    use crate::multi_stream_reader::StreamSource;
    use crate::naive_hyper_pattern_matching::NaiveHyperPatternMatching;
    use crate::result_notifier::{MatchingInterval, MatchingResult, SharedBufferResultNotifier};
    use crate::shared_buffer::{SharedBuffer, SharedBufferSource};
    use std::collections::HashSet;
    use typed_arena::Arena;

    #[test]
    fn test_run() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = Automata::new(&state_arena, &transition_arena, 2);

        let s1 = automaton.add_state(true, false);
        let s12 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s13 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_nfah_transition(s1, "a".to_string(), 0, s12);
        automaton.add_nfah_transition(s12, "b".to_string(), 1, s2);
        automaton.add_nfah_transition(s1, "a".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "b".to_string(), 1, s1);
        automaton.add_nfah_transition(s1, "c".to_string(), 0, s13);
        automaton.add_nfah_transition(s13, "d".to_string(), 1, s3);

        let input_buffers = vec![SharedBuffer::new(), SharedBuffer::new()];
        let input_buffers_source: Vec<SharedBufferSource<&str>> =
            input_buffers.iter().map(|buf| buf.make_source()).collect();
        let reader = MultiStreamReader::new(
            input_buffers
                .clone()
                .into_iter()
                .map(|buf| Box::new(buf) as Box<dyn StreamSource>)
                .collect(),
        );

        let result_buffer = SharedBuffer::new();
        let notifier = SharedBufferResultNotifier::new(result_buffer.make_source());
        let mut result_sink = result_buffer.make_sink();

        let matching = NaiveHyperPatternMatching::<SharedBufferResultNotifier>::new(
            &automaton,
            notifier,
            vec![AppendOnlySequence::new(), AppendOnlySequence::new()],
        );

        let mut scheduler = ReadingScheduler::new(matching, reader);

        input_buffers_source[0].push("a");
        input_buffers_source[0].push("a");
        input_buffers_source[0].push("c");

        input_buffers_source[1].push("a");
        input_buffers_source[1].push("d");
        input_buffers_source[1].push("d");

        scheduler.run();

        let mut results = HashSet::new();
        let mut result = result_sink.pop();
        while result.is_some() {
            results.insert(result.unwrap());
            result = result_sink.pop();
        }

        assert_eq!(results.len(), 6);
        assert!(results.contains(&MatchingResult {
            intervals: vec![MatchingInterval::new(0, 2), MatchingInterval::new(1, 1)],
            ids: vec![0, 1]
        }));
        assert!(results.contains(&MatchingResult {
            intervals: vec![MatchingInterval::new(1, 2), MatchingInterval::new(1, 1)],
            ids: vec![0, 1]
        }));
        assert!(results.contains(&MatchingResult {
            intervals: vec![MatchingInterval::new(2, 2), MatchingInterval::new(1, 1)],
            ids: vec![0, 1]
        }));
        assert!(results.contains(&MatchingResult {
            intervals: vec![MatchingInterval::new(0, 2), MatchingInterval::new(2, 2)],
            ids: vec![0, 1]
        }));
        assert!(results.contains(&MatchingResult {
            intervals: vec![MatchingInterval::new(1, 2), MatchingInterval::new(2, 2)],
            ids: vec![0, 1]
        }));
        assert!(results.contains(&MatchingResult {
            intervals: vec![MatchingInterval::new(2, 2), MatchingInterval::new(2, 2)],
            ids: vec![0, 1]
        }));
    }
}
