use std::{collections::HashMap, marker::PhantomData};

use itertools::Itertools;
use typed_arena::Arena;

use crate::{
    automata::NFAH, automata_runner::AppendOnlySequence,
    dfa_earliest_pattern_matcher::DFAEarliestPatternMatcher,
    filtered_single_hyper_pattern_matching::FilteredSingleHyperPatternMatching,
    hyper_pattern_matching::HyperPatternMatching, matching_filter::MatchingFilter,
    result_notifier::ResultNotifier,
};

pub struct FilteredHyperPatternMatching<'a, SingleMatching, Notifier>
where
    SingleMatching: FilteredSingleHyperPatternMatching<'a, Notifier>,
    Notifier: ResultNotifier + Clone,
{
    automaton: &'a NFAH<'a>,
    filters: HashMap<(usize, usize), MatchingFilter<usize, String>>,
    single_matchings: Vec<SingleMatching>,
    sequences: Vec<AppendOnlySequence<String>>,
    _notifier: PhantomData<Notifier>,
}

impl<'a, SingleMatching, Notifier> FilteredHyperPatternMatching<'a, SingleMatching, Notifier>
where
    SingleMatching: FilteredSingleHyperPatternMatching<'a, Notifier>,
    Notifier: ResultNotifier + Clone,
{
    pub fn new(
        automaton: &'a NFAH<'a>,
        notifier: Notifier,
        sequences: Vec<AppendOnlySequence<String>>,
    ) -> Self {
        let mut filters = HashMap::with_capacity(automaton.dimensions * sequences.len());
        let enfa_state_arena = Arena::new();
        let enfa_transition_arena = Arena::new();
        let nfa_state_arena = Arena::new();
        let nfa_transition_arena = Arena::new();
        let mut dfas = Vec::with_capacity(automaton.dimensions);
        for variable in 0..automaton.dimensions {
            dfas.push(
                automaton
                    .project(&enfa_state_arena, &enfa_transition_arena, variable)
                    .to_nfa_powerset(&nfa_state_arena, &nfa_transition_arena)
                    .determinize(),
            );
        }
        for variable in 0..automaton.dimensions {
            for id in 0..sequences.len() {
                let dfa_matcher = DFAEarliestPatternMatcher::new(dfas[variable].clone());
                filters.insert(
                    (variable, id),
                    MatchingFilter::new(dfa_matcher, sequences[id].readable_view()),
                );
            }
        }

        let ranges = vec![0..sequences.len(); automaton.dimensions];
        let ids = ranges.into_iter().multi_cartesian_product().collect_vec();
        let mut single_matchings = Vec::with_capacity(ids.len());
        for id in &ids {
            let mut input_streams = Vec::with_capacity(id.len());
            for i in 0..id.len() {
                let variable = id[i];
                if let Some(filter) = filters.get(&(variable, i)) {
                    input_streams.push(filter.readable_view());
                } else {
                    panic!("No filter found for variable {} and id {}", variable, i);
                }
            }
            single_matchings.push(SingleMatching::new(
                automaton,
                notifier.clone(),
                input_streams,
                id.clone(),
            ));
        }

        Self {
            automaton,
            filters,
            single_matchings,
            sequences,
            _notifier: PhantomData,
        }
    }

    fn consume(&mut self) {
        // Run the filters
        for variable in 0..self.automaton.dimensions {
            for id in 0..self.sequences.len() {
                self.filters
                    .get_mut(&(variable, id))
                    .unwrap()
                    .consume_input();
            }
        }
        // Run the matchers
        for single_matching in self.single_matchings.iter_mut() {
            single_matching.consume_input();
        }
    }
}

impl<'a, SingleMatching, Notifier> HyperPatternMatching
    for FilteredHyperPatternMatching<'a, SingleMatching, Notifier>
where
    SingleMatching: FilteredSingleHyperPatternMatching<'a, Notifier>,
    Notifier: ResultNotifier + Clone,
{
    fn feed(&mut self, action: &str, track: usize) {
        self.sequences[track].append(action.to_string());
        // Run the filters
        for variable in 0..self.automaton.dimensions {
            self.filters
                .get_mut(&(variable, track))
                .unwrap()
                .consume_input();
        }
        // Run the matchers
        for single_matching in self.single_matchings.iter_mut() {
            single_matching.consume_input();
        }
    }

    fn dimensions(&self) -> usize {
        self.automaton.dimensions
    }

    fn consume_remaining(&mut self) {
        self.consume();
    }

    fn set_eof(&mut self, track: usize) {
        self.sequences[track].close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filtered_single_hyper_pattern_matching::NaiveFilteredSingleHyperPatternMatching;
    use crate::multi_stream_reader::{MultiStreamReader, StreamSource};
    use crate::reading_scheduler::ReadingScheduler;
    use crate::result_notifier::{MatchingInterval, SharedBufferResultNotifier};
    use crate::shared_buffer::SharedBuffer;
    use typed_arena::Arena;

    #[test]
    fn test_run() {
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

        let matching = FilteredHyperPatternMatching::<
            NaiveFilteredSingleHyperPatternMatching<SharedBufferResultNotifier>,
            SharedBufferResultNotifier,
        >::new(
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
            assert_eq!(result.intervals[1], MatchingInterval::new(2, 2));
            assert_eq!(result.ids, vec![0, 1]);
        }
    }
}
