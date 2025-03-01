use std::{collections::HashMap, marker::PhantomData};

use itertools::Itertools;
use log::debug;
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
    pub fn new(automaton: &'a NFAH<'a>, notifier: Notifier, dimensions: usize) -> Self {
        let sequences = (0..dimensions)
            .map(|_| AppendOnlySequence::new())
            .collect_vec();
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

    pub fn consume(&mut self) {
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

        // Apply check_closed for each filter
        self.filters
            .values_mut()
            .for_each(|filter| filter.check_closed());

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
        debug!("FilteredHyperPatternMatching::set_eof({})", track);
        self.sequences[track].close();
    }
}
