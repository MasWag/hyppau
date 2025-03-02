use std::collections::BTreeSet;

use itertools::Itertools;
use log::{debug, trace};

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

    /// Returns the indices where the current value of the stream is None
    fn skipped_streams(&self, start_position: &StartPosition) -> Vec<usize> {
        let mut skipped_streams = Vec::with_capacity(start_position.start_indices.len());

        for i in 0..start_position.start_indices.len() {
            let stream_len = self.get_input_stream(i).len();
            let start_index = start_position.start_indices[i];
            let readable_slice = self.get_input_stream(i).readable_slice();
            if start_index < stream_len && readable_slice[start_index].is_none() {
                skipped_streams.push(i);
            }
        }
        skipped_streams
    }
}

pub struct NaiveFilteredSingleHyperPatternMatching<'a, Notifier: ResultNotifier> {
    automata_runner: FilteredPatternMatchingAutomataRunner<'a>,
    notifier: Notifier,
    input_streams: Vec<ReadableView<Option<String>>>,
    ids: Vec<usize>,
    waiting_queue: BTreeSet<StartPosition>,
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
        let mut automata_runner = FilteredPatternMatchingAutomataRunner::new(
            automaton,
            input_streams.clone(),
            ids.clone(),
        );
        let start_indices = vec![0; automaton.dimensions];
        let waiting_queue = StartPosition { start_indices }
            .immediate_successors()
            .into_iter()
            .collect();

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
        trace!("Enter NaiveFilteredSingleHyperPatternMatching::consume_input");
        self.automata_runner.consume();
        let final_configurations = self.automata_runner.get_final_configurations();
        let dimensions = self.dimensions();
        final_configurations.iter().cloned().for_each(|c| {
            assert_eq!(c.ids, self.ids);
            let mut intervals = Vec::with_capacity(dimensions);
            for i in 0..dimensions {
                let begin = c.matching_begin[i];
                let end = c.input_sequence[i].start - 1;
                intervals.push(MatchingInterval::new(begin, end));
            }
            self.notifier.notify(&intervals, &c.ids);
        });
        self.automata_runner.remove_non_waiting_configurations();
        self.automata_runner.remove_masked_configurations();
        while self.automata_runner.current_configurations.is_empty() {
            while self.automata_runner.current_configurations.is_empty() {
                let new_position = self.waiting_queue.pop_first();
                trace!("new_position: {:?}", new_position);
                // Start new matching trial
                if let Some(new_position) = new_position {
                    debug!(
                        "new_position is {}",
                        (if self.is_skipped(&new_position) {
                            "skipped"
                        } else {
                            "not skipped"
                        })
                    );
                    let valid_successors = self.compute_valid_successors(&new_position);

                    // Put the successors to the waiting queue
                    self.waiting_queue.extend(valid_successors);

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
                    trace!("Exit NaiveFilteredSingleHyperPatternMatching::consume_input");
                    return;
                }
            }
            self.automata_runner.consume();
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
            // self.automata_runner.remove_masked_configurations();
        }
        trace!("Exit NaiveFilteredSingleHyperPatternMatching::consume_input");
    }

    fn get_input_stream(&self, variable: usize) -> &ReadableView<Option<String>> {
        if let Some(stream) = self.input_streams.get(variable) {
            stream
        } else {
            panic!("Variable {} is out of range", variable);
        }
    }
}

impl<Notifier: ResultNotifier> NaiveFilteredSingleHyperPatternMatching<'_, Notifier> {
    fn compute_valid_successors(&self, start_position: &StartPosition) -> Vec<StartPosition> {
        let mut waiting_queue = Vec::new();
        waiting_queue.push(start_position.clone());
        let mut valid_successors = Vec::new();
        while let Some(examined_position) = waiting_queue.pop() {
            let skipped_streams = self.skipped_streams(&examined_position);

            examined_position
                .immediate_successors()
                .into_iter()
                .filter(|successor| self.in_range(successor))
                .for_each(|successor| {
                    // trace!("successor: {:?}", successor);
                    if self.is_skipped(&successor) {
                        if skipped_streams.is_empty()
                            || skipped_streams.iter().any(|&i| {
                                successor.start_indices[i] != examined_position.start_indices[i]
                            })
                        {
                            // trace!("pushed to waiting_queue");
                            waiting_queue.push(successor);
                        } else {
                            // trace!("skipped index must be updated {:?} -> {:?} ({:?})", examined_position, successor, skipped_streams);
                        }
                    } else {
                        // trace!("pushed to valid_successors");
                        valid_successors.push(successor);
                    }
                });
        }

        valid_successors
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::{
        automata_runner::AppendOnlySequence,
        result_notifier::{MatchingResult, SharedBufferResultNotifier},
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

    #[test]
    fn test_small_double() {
        // Create the automaton in small.json
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

        // Create states based on small.json
        let s0 = automaton.add_state(true, false); // id: 0, is_initial: true, is_final: false
        let s1 = automaton.add_state(false, false); // id: 1, is_initial: false, is_final: false
        let s2 = automaton.add_state(false, false); // id: 2, is_initial: false, is_final: false
        let s3 = automaton.add_state(false, false); // id: 3, is_initial: false, is_final: false
        let s4 = automaton.add_state(false, true); // id: 4, is_initial: false, is_final: true

        // Add transitions based on small.json
        automaton.add_nfah_transition(s0, "a".to_string(), 0, s1); // from: 0, to: 1, label: ["a", 0]
        automaton.add_nfah_transition(s1, "b".to_string(), 1, s2); // from: 1, to: 2, label: ["b", 1]
        automaton.add_nfah_transition(s0, "a".to_string(), 0, s0); // from: 0, to: 0, label: ["a", 0]
        automaton.add_nfah_transition(s0, "b".to_string(), 1, s0); // from: 0, to: 0, label: ["b", 1]
        automaton.add_nfah_transition(s0, "c".to_string(), 0, s3); // from: 0, to: 3, label: ["c", 0]
        automaton.add_nfah_transition(s3, "d".to_string(), 1, s4); // from: 3, to: 4, label: ["d", 1]

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
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

        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[1].append(None);
        matcher.consume_input();
        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[1].append(Some("d".to_string()));
        matcher.consume_input();
        sequences[0].append(Some("c".to_string()));
        matcher.consume_input();
        sequences[1].append(Some("d".to_string()));
        matcher.consume_input();
        sequences[1].close();
        matcher.consume_input();
        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[0].append(Some("a".to_string()));
        matcher.consume_input();
        sequences[0].append(Some("c".to_string()));
        matcher.consume_input();
        sequences[0].close();
        matcher.consume_input();

        // The expected results
        let expected_results = vec![
            (0, 2, 1, 1),
            (0, 2, 2, 2),
            (1, 2, 1, 1),
            (1, 2, 2, 2),
            (2, 2, 1, 1),
            (2, 2, 2, 2),
            (3, 5, 1, 1),
            (3, 5, 2, 2),
            (4, 5, 1, 1),
            (4, 5, 2, 2),
            (5, 5, 1, 1),
            (5, 5, 2, 2),
        ]
        .iter()
        .map(|(s1, e1, s2, e2)| {
            MatchingResult::new(
                vec![
                    MatchingInterval::new(*s1, *e1),
                    MatchingInterval::new(*s2, *e2),
                ],
                vec![0, 1],
            )
        })
        .collect_vec();

        // Collect all results
        let mut results = HashSet::new();
        while let Some(result) = result_sink.pop() {
            results.insert(result);
        }

        assert_eq!(results.len(), expected_results.len());

        for expected_result in expected_results {
            assert!(results.contains(&expected_result));
        }
    }
}
