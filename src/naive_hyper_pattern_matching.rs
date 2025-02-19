use crate::automata::Automata;
use crate::automata_runner::{AppendOnlySequence, AutomataRunner};
use crate::hyper_pattern_matching::{HyperPatternMatching, PatternMatchingAutomataRunner};
use crate::result_notifier::{MatchingInterval, ResultNotifier};
use itertools::Itertools;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// the element in the waiting queue of hyper pattern matching algorithms based on priority-queue.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct StartPosition {
    /// The starting indices of the word in the pattern.
    pub start_indices: Vec<usize>,
}

impl StartPosition {
    fn immediate_successors(&self) -> Vec<StartPosition> {
        let mut result = Vec::with_capacity(self.start_indices.len());
        for i in 0..self.start_indices.len() {
            let mut new_start_indices = self.start_indices.clone();
            new_start_indices[i] += 1;
            result.push(StartPosition {
                start_indices: new_start_indices,
            });
        }

        return result;
    }
}

pub struct NaiveHyperPatternMatching<'a, Notifier: ResultNotifier> {
    automata_runner: PatternMatchingAutomataRunner<'a>,
    notifier: Notifier,
    sequences: Vec<AppendOnlySequence<String>>,
    read_size: Vec<usize>,
    waiting_queues: HashMap<Vec<usize>, BinaryHeap<Reverse<StartPosition>>>,
    /// Either we reached the end of the sequences
    eof: Vec<bool>,
}

impl<'a, Notifier: ResultNotifier> NaiveHyperPatternMatching<'a, Notifier> {
    pub fn new(
        automaton: &'a Automata<'a>,
        notifier: Notifier,
        sequences: Vec<AppendOnlySequence<String>>,
    ) -> Self {
        let as_readable_view = sequences.iter().map(|s| s.readable_view()).collect();
        let mut automata_runner = PatternMatchingAutomataRunner::new(automaton, as_readable_view);
        let read_size = vec![0; sequences.len()];
        let eof = vec![false; sequences.len()];
        let start_indices = vec![0; automaton.dimensions];
        let successors = StartPosition { start_indices }
            .immediate_successors()
            .into_iter()
            .map(|p| Reverse(p))
            .collect_vec();
        let ranges = vec![0..sequences.len(); automaton.dimensions];
        let ids = ranges.into_iter().multi_cartesian_product().collect_vec();
        let waiting_queue = BinaryHeap::from(successors);
        let mut waiting_queues = HashMap::with_capacity(ids.len());
        for id in ids {
            let input_sequence = id
                .iter()
                .map(|i| {
                    let mut view = sequences[*i].readable_view();
                    view.advance_readable(0);
                    view
                })
                .collect_vec();
            waiting_queues.insert(id, waiting_queue.clone());
            automata_runner.insert_from_initial_states(input_sequence);
        }

        Self {
            automata_runner,
            notifier,
            sequences,
            read_size,
            waiting_queues,
            eof,
        }
    }

    pub fn in_range(&self, start_position: &StartPosition) -> bool {
        for i in 0..self.sequences.len() {
            if self.eof[i] && start_position.start_indices[i] >= self.read_size[i] {
                return false;
            }
        }
        true
    }
}

impl<'a, Notifier: ResultNotifier> HyperPatternMatching
    for NaiveHyperPatternMatching<'a, Notifier>
{
    fn feed(&mut self, action: &str, track: usize) {
        self.sequences[track].append(action.to_string());
        self.read_size[track] += 1;
        self.automata_runner.consume();
        let final_configurations = self.automata_runner.get_final_configurations();
        let dimensions = self.dimensions();
        final_configurations.iter().for_each(|c| {
            let mut intervals = Vec::with_capacity(dimensions);
            for i in 0..dimensions {
                let begin = c.matching_begin[i];
                let end = c.input_sequence[i].start - 1;
                intervals.push(MatchingInterval::new(begin, end));
            }
            self.notifier.notify(&intervals, &c.ids);
        });
        self.automata_runner.remove_non_waiting_configurations();
        let current_ids = self
            .automata_runner
            .current_configurations
            .iter()
            .map(|c| c.ids.clone())
            .collect::<HashSet<Vec<usize>>>();
        let keys = self.waiting_queues.keys().cloned().collect_vec();
        for id in keys {
            if !current_ids.contains(&id) {
                let new_position = {
                    let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                    waiting_queue.pop()
                };
                // Start new matching trial
                if let Some(new_position) = new_position {
                    let valid_successors = new_position
                        .0
                        .immediate_successors()
                        .into_iter()
                        .filter(|successor| self.in_range(successor))
                        .collect_vec();
                    // Put the successors to the waiting queue
                    for successor in valid_successors {
                        let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                        waiting_queue.push(Reverse(successor))
                    }
                    let input_sequence = id
                        .into_iter()
                        .map(|i| {
                            let mut view = self.sequences[i].readable_view();
                            view.advance_readable(new_position.0.start_indices[i]);
                            view
                        })
                        .collect_vec();
                    self.automata_runner
                        .insert_from_initial_states(input_sequence)
                }
            }
        }
    }

    fn dimensions(&self) -> usize {
        self.sequences.len()
    }

    fn consume_remaining(&mut self) {
        while self.waiting_queues.values().any(|f| f.len() > 0) {
            self.automata_runner.consume();
            let final_configurations = self.automata_runner.get_final_configurations();
            let dimensions = self.dimensions();
            final_configurations.iter().for_each(|c| {
                let mut intervals = Vec::with_capacity(dimensions);
                for i in 0..dimensions {
                    let begin = c.matching_begin[i];
                    let end = c.input_sequence[i].start - 1;
                    intervals.push(MatchingInterval::new(begin, end));
                }
                self.notifier.notify(&intervals, &c.ids);
            });
            self.automata_runner.current_configurations.clear();
            let keys = self.waiting_queues.keys().cloned().collect_vec();
            for id in keys {
                let new_position = {
                    let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                    waiting_queue.pop()
                };
                // Start new matching trial
                if let Some(new_position) = new_position {
                    let valid_successors = new_position
                        .0
                        .immediate_successors()
                        .into_iter()
                        .filter(|successor| self.in_range(successor))
                        .collect_vec();
                    // Put the successors to the waiting queue
                    for successor in valid_successors {
                        let waiting_queue = self.waiting_queues.get_mut(&id).unwrap();
                        waiting_queue.push(Reverse(successor))
                    }
                    let input_sequence = id
                        .into_iter()
                        .map(|i| {
                            let mut view = self.sequences[i].readable_view();
                            view.advance_readable(new_position.0.start_indices[i]);
                            view
                        })
                        .collect_vec();
                    self.automata_runner
                        .insert_from_initial_states(input_sequence)
                }
            }
        }
    }

    fn set_eof(&mut self, track: usize) {
        self.eof[track] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multi_stream_reader::{MultiStreamReader, StreamSource};
    use crate::reading_scheduler::ReadingScheduler;
    use crate::result_notifier::SharedBufferResultNotifier;
    use crate::shared_buffer::SharedBuffer;
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

        automaton.add_transition(s1, "a".to_string(), 0, s12);
        automaton.add_transition(s12, "b".to_string(), 1, s2);
        automaton.add_transition(s1, "a".to_string(), 0, s1);
        automaton.add_transition(s1, "b".to_string(), 1, s1);
        automaton.add_transition(s1, "c".to_string(), 0, s13);
        automaton.add_transition(s13, "d".to_string(), 1, s3);

        let input_buffers = vec![SharedBuffer::new(), SharedBuffer::new()];
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

        let matching = NaiveHyperPatternMatching::new(
            &automaton,
            notifier,
            vec![AppendOnlySequence::new(), AppendOnlySequence::new()],
        );

        let mut scheduler = ReadingScheduler::new(matching, reader);

        input_buffers[0].push("a");
        input_buffers[1].push("b");
        input_buffers[0].push("a");
        input_buffers[1].push("b");
        input_buffers[0].push("c");
        input_buffers[1].push("d");

        scheduler.run();
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(0, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(0, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(0, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(0, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(0, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(0, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(0, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(1, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(2, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(0, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(2, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(2, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(2, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(1, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
        {
            let result = result_sink.pop().expect("No data in shared buffer");
            assert_eq!(result.intervals.len(), 2);
            assert_eq!(result.ids.len(), 2);
            assert_eq!(result.intervals[0], MatchingInterval::new(2, 2));
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
    }
}
