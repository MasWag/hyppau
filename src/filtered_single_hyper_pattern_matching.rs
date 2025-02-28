use itertools::Itertools;

use crate::{
    automata::NFAH,
    automata_runner::ReadableView,
    filtered_pattern_matching_automata_runner::FilteredPatternMatchingAutomataRunner,
    naive_hyper_pattern_matching::StartPosition,
    result_notifier::{MatchingInterval, ResultNotifier},
};

/// Trait of the algorithms for hyper pattern matching, where the word assignment is already fixed.
pub trait FilteredSingleHyperPatternMatching<'a, Notifier: ResultNotifier> {
    fn new(
        automaton: &'a NFAH<'a>,
        notifier: Notifier,
        input_streams: Vec<ReadableView<Option<String>>>,
        ids: Vec<usize>,
    ) -> Self;

    /// The number of variables in hyper pattern matching.
    fn dimensions(&self) -> usize;

    /// Returns the word id of the given variable
    fn get_id(&self, variable: usize) -> Option<usize>;

    /// Consumes elements from the input stream, conduct single hyper pattern matching, and notify the detected matching.
    fn consume_input(&mut self);

    fn get_input_stream(&self, variable: usize) -> &ReadableView<Option<String>>;

    /// Check if the given start position is within the range of the input streams.
    fn in_range(&self, start_position: &StartPosition) -> bool {
        for i in 0..start_position.start_indices.len() {
            if self.get_input_stream(i).is_closed()
                && start_position.start_indices[i] >= self.get_input_stream(i).len()
            {
                return false;
            }
        }
        true
    }

    /// Check if the word of the given start position is not skipped.
    fn is_skipped(&self, start_position: &StartPosition) -> bool {
        for i in 0..start_position.start_indices.len() {
            let start_index = start_position.start_indices[i];
            let readable_slice = self.get_input_stream(i).readable_slice();
            if start_index < self.get_input_stream(i).len() && readable_slice[start_index].is_none()
            {
                return true;
            }
        }
        false
    }
}

pub struct NaiveFilteredSingleHyperPatternMatching<'a, Notifier: ResultNotifier> {
    automata_runner: FilteredPatternMatchingAutomataRunner<'a>,
    notifier: Notifier,
    input_streams: Vec<ReadableView<Option<String>>>,
    ids: Vec<usize>,
    waiting_queue: Vec<StartPosition>,
}

impl<'a, Notifier: ResultNotifier> FilteredSingleHyperPatternMatching<'a, Notifier>
    for NaiveFilteredSingleHyperPatternMatching<'a, Notifier>
{
    fn new(
        automaton: &'a NFAH<'a>,
        notifier: Notifier,
        input_streams: Vec<ReadableView<Option<String>>>,
        ids: Vec<usize>,
    ) -> Self {
        let mut automata_runner =
            FilteredPatternMatchingAutomataRunner::new(automaton, input_streams.clone());
        let start_indices = vec![0; automaton.dimensions];
        let mut waiting_queue = StartPosition { start_indices }
            .immediate_successors()
            .into_iter()
            .collect_vec();
        waiting_queue.sort_by(|a, b| a.cmp(b).reverse());
        automata_runner.insert_from_initial_states(input_streams.clone());

        Self {
            automata_runner,
            notifier,
            input_streams,
            ids,
            waiting_queue,
        }
    }

    fn dimensions(&self) -> usize {
        self.ids.len()
    }

    fn get_id(&self, variable: usize) -> Option<usize> {
        self.ids.get(variable).copied()
    }

    fn consume_input(&mut self) {
        while self.automata_runner.consume() {
            let final_configurations = self.automata_runner.get_final_configurations();
            let dimensions = self.dimensions();
            final_configurations.iter().cloned().for_each(|c| {
                let mut intervals = Vec::with_capacity(dimensions);
                for i in 0..dimensions {
                    let begin = c.matching_begin[i];
                    let end = c.input_sequence[i].start - 1;
                    intervals.push(MatchingInterval::new(begin, end));
                }
                self.notifier.notify(&intervals, &c.ids);
            });
            self.automata_runner.remove_non_waiting_configurations();
            while self.automata_runner.current_configurations.is_empty() {
                let new_position = self.waiting_queue.pop();
                // Start new matching trial
                if let Some(new_position) = new_position {
                    let mut valid_successors = new_position
                        .immediate_successors()
                        .into_iter()
                        .filter(|successor| self.in_range(successor))
                        .collect_vec();
                    // Put the successors to the waiting queue
                    self.waiting_queue.append(&mut valid_successors);
                    // Optimization: we can optimize here by not pushing the skipped elements
                    self.waiting_queue.sort_by(|a, b| a.cmp(b).reverse());
                    self.waiting_queue.dedup();
                    if !self.is_skipped(&new_position) {
                        let mut input_streams = self.input_streams.clone();
                        for variable in 0..dimensions {
                            input_streams[variable]
                                .advance_readable(new_position.start_indices[variable]);
                        }
                        self.automata_runner
                            .insert_from_initial_states(input_streams);
                    }
                } else {
                    break;
                }
            }
        }
    }

    fn get_input_stream(&self, variable: usize) -> &ReadableView<Option<String>> {
        if let Some(stream) = self.input_streams.get(variable) {
            stream
        } else {
            panic!("Variable {} is out of range", variable);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        automata_runner::AppendOnlySequence, result_notifier::SharedBufferResultNotifier,
        shared_buffer::SharedBuffer,
    };
    use typed_arena::Arena;

    #[test]
    fn test_single_hyper_pattern_matching() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

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
        automaton.remove_unreachable_transitions();

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append(Some("a".to_string()));
        sequences[1].append(Some("b".to_string()));
        sequences[0].append(Some("c".to_string()));
        sequences[1].append(Some("d".to_string()));
        sequences[0].append(None);
        sequences[1].append(None);
        sequences[0].append(Some("c".to_string()));
        sequences[1].append(Some("d".to_string()));
        sequences[0].close();
        sequences[1].close();
        let input_streams = sequences.iter().map(|s| s.readable_view()).collect();
        let ids = vec![0, 1];

        let result_buffer = SharedBuffer::new();

        let mut matcher = NaiveFilteredSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            input_streams,
            ids.clone(),
        );

        let mut result_sink = result_buffer.make_sink();
        matcher.consume_input();

        // Test the results
        let expected_results = vec![
            vec![MatchingInterval::new(0, 1), MatchingInterval::new(0, 1)],
            vec![MatchingInterval::new(0, 1), MatchingInterval::new(1, 1)],
            vec![MatchingInterval::new(0, 1), MatchingInterval::new(3, 3)],
            vec![MatchingInterval::new(1, 1), MatchingInterval::new(0, 1)],
            vec![MatchingInterval::new(1, 1), MatchingInterval::new(1, 1)],
            vec![MatchingInterval::new(1, 1), MatchingInterval::new(3, 3)],
            vec![MatchingInterval::new(3, 3), MatchingInterval::new(0, 1)],
            vec![MatchingInterval::new(3, 3), MatchingInterval::new(1, 1)],
            vec![MatchingInterval::new(3, 3), MatchingInterval::new(3, 3)],
        ];
        for expected_result in expected_results {
            let result = result_sink.pop();
            println!("{:?}", result);
            assert!(result.is_some());
            assert_eq!(ids.clone(), result.clone().unwrap().ids);
            assert_eq!(expected_result, result.unwrap().intervals);
        }
        assert!(result_sink.pop().is_none());
    }
}
