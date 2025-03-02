use std::cmp::Reverse;

use itertools::Itertools;

use crate::{
    automata::NFAH,
    automata_runner::{NFAHRunner, ReadableView},
    hyper_pattern_matching::PatternMatchingAutomataRunner,
    kmp_skip_values::KMPSkipValues,
    naive_hyper_pattern_matching::StartPosition,
    quick_search_skip_values::QuickSearchSkipValues,
    result_notifier::{MatchingInterval, ResultNotifier},
    single_hyper_pattern_matching::SingleHyperPatternMatching,
};

pub struct FJSSingleHyperPatternMatching<'a, Notifier: ResultNotifier> {
    automata_runner: PatternMatchingAutomataRunner<'a>,
    notifier: Notifier,
    input_streams: Vec<ReadableView<String>>,
    ids: Vec<usize>,
    waiting_queue: Vec<Reverse<StartPosition>>,
    /// The set of ignored starting positions by the skip values
    skipped_positions: Vec<Vec<usize>>,
    quick_search_skip_value: QuickSearchSkipValues,
    kmp_skip_value: KMPSkipValues<'a>,
}

impl<'a, Notifier: ResultNotifier> SingleHyperPatternMatching<'a, Notifier>
    for FJSSingleHyperPatternMatching<'a, Notifier>
{
    fn new(
        automaton: &'a NFAH<'a>,
        notifier: Notifier,
        input_streams: Vec<ReadableView<String>>,
        ids: Vec<usize>,
    ) -> Self {
        let mut automata_runner =
            PatternMatchingAutomataRunner::new(automaton, input_streams.clone());
        let start_indices = vec![0; automaton.dimensions];
        let mut waiting_queue = StartPosition { start_indices }
            .immediate_successors()
            .into_iter()
            .map(Reverse)
            .collect_vec();
        waiting_queue.sort();
        automata_runner.insert_from_initial_states(input_streams.clone(), ids.clone());

        let skipped_positions = (0..automaton.dimensions).map(|_| Vec::new()).collect_vec();

        Self {
            automata_runner,
            notifier,
            input_streams,
            ids,
            waiting_queue,
            skipped_positions,
            quick_search_skip_value: QuickSearchSkipValues::new(automaton),
            kmp_skip_value: KMPSkipValues::new(automaton),
        }
    }

    fn dimensions(&self) -> usize {
        self.ids.len()
    }

    fn get_id(&self, variable: usize) -> Option<usize> {
        self.ids.get(variable).copied()
    }

    fn consume_input(&mut self) {
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

        // Apply KMP-style skip values
        for c in &self.automata_runner.current_configurations {
            for i in 0..c.ids.len() {
                if let Some(&skip_value) = self.kmp_skip_value.skip_values[i].get(c.current_state) {
                    for j in 1..skip_value {
                        if i < self.skipped_positions.len() {
                            self.skipped_positions[i].push(c.matching_begin[i] + j);
                        }
                    }
                }
            }
        }

        self.automata_runner.remove_non_waiting_configurations();

        while self.automata_runner.is_empty() {
            // Find a new valid starting position
            if let Some(position) = self.waiting_queue.pop() {
                let mut valid_successors = self.compute_valid_successors(&position.0);

                // Put the successors to the waiting queue
                self.waiting_queue.append(&mut valid_successors);
                self.waiting_queue.sort();
                self.waiting_queue.dedup();

                if self.is_valid_position(&position.0) {
                    let mut input_streams = self.input_streams.clone();
                    for variable in 0..self.dimensions() {
                        input_streams[variable]
                            .advance_readable(position.0.start_indices[variable]);
                    }

                    self.automata_runner
                        .insert_from_initial_states(input_streams, self.ids.clone());
                    self.automata_runner.consume();

                    let final_configurations = self.automata_runner.get_final_configurations();
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
                }
            } else {
                // No more valid positions to try
                break;
            }
        }
    }

    fn get_input_stream(&self, variable: usize) -> &ReadableView<String> {
        if let Some(stream) = self.input_streams.get(variable) {
            stream
        } else {
            panic!("Variable {} is out of range", variable);
        }
    }
}

impl<Notifier: ResultNotifier> FJSSingleHyperPatternMatching<'_, Notifier> {
    /// Check if a position is valid (not skipped and within range)
    fn is_valid_position(&self, position: &StartPosition) -> bool {
        // Check if position is within range
        for i in 0..position.start_indices.len() {
            let stream = self.get_input_stream(i);
            if stream.is_closed() && position.start_indices[i] >= stream.len() {
                return false;
            }
        }

        // Check if position is skipped
        for i in 0..position.start_indices.len() {
            if i < self.skipped_positions.len()
                && self.skipped_positions[i].contains(&position.start_indices[i])
            {
                return false;
            }
        }

        true
    }

    fn compute_valid_successors(
        &self,
        start_position: &StartPosition,
    ) -> Vec<Reverse<StartPosition>> {
        let mut waiting_queue = Vec::new();
        waiting_queue.push(start_position.clone());
        let mut valid_successors = Vec::new();
        while let Some(examined_position) = waiting_queue.pop() {
            examined_position
                .immediate_successors()
                .into_iter()
                .filter(|successor| self.in_range(successor))
                .for_each(|successor| {
                    if self.is_valid_position(&successor) {
                        valid_successors.push(Reverse(successor));
                    } else {
                        waiting_queue.push(successor);
                    }
                });
        }

        valid_successors
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
    fn test_fjs_single_hyper_pattern_matching() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &transition_arena, 2);

        let s0 = automaton.add_state(true, false);
        let s1 = automaton.add_state(false, false);
        let s12 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s13 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_nfah_transition(s0, "a".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "a".to_string(), 0, s12);
        automaton.add_nfah_transition(s12, "b".to_string(), 1, s2);
        automaton.add_nfah_transition(s1, "a".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "b".to_string(), 1, s1);
        automaton.add_nfah_transition(s1, "c".to_string(), 0, s13);
        automaton.add_nfah_transition(s13, "d".to_string(), 1, s3);
        automaton.remove_unreachable_transitions();

        let mut sequences = [AppendOnlySequence::new(), AppendOnlySequence::new()];
        sequences[0].append("e".to_string());
        sequences[1].append("e".to_string());
        sequences[0].append("e".to_string());
        sequences[1].append("e".to_string());
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].append("c".to_string());
        sequences[1].append("d".to_string());
        sequences[0].append("a".to_string());
        sequences[1].append("b".to_string());
        sequences[0].close();
        sequences[1].close();
        let input_streams = sequences.iter().map(|s| s.readable_view()).collect();
        let ids = vec![0, 1];

        let result_buffer = SharedBuffer::new();

        let mut matcher = FJSSingleHyperPatternMatching::new(
            &automaton,
            SharedBufferResultNotifier::new(result_buffer.make_source()),
            input_streams,
            ids.clone(),
        );

        let mut result_sink = result_buffer.make_sink();
        matcher.consume_input();

        // Test the results - collect all results first
        let mut actual_results = Vec::new();
        while let Some(result) = result_sink.pop() {
            assert_eq!(ids.clone(), result.ids);
            actual_results.push(result.intervals);
        }

        // Check that we have the expected number of results
        assert_eq!(2, actual_results.len());

        // Check that all expected patterns are present
        let expected_patterns = vec![
            vec![MatchingInterval::new(2, 3), MatchingInterval::new(2, 3)],
            vec![MatchingInterval::new(2, 3), MatchingInterval::new(3, 3)],
        ];

        for expected in expected_patterns {
            assert!(
                actual_results.contains(&expected),
                "Expected result {:?} not found in actual results",
                expected
            );
        }
        assert!(result_sink.pop().is_none());
    }
}
