use itertools::{Itertools, Product};
use std::collections::{HashMap, HashSet, VecDeque};
use typed_arena::Arena;

use crate::automata::{NFAHState, NFAState, NFAH};

pub trait VectorWithVer<T> {
    fn project(&self, variable: usize) -> Vec<T>;
}

impl<T> VectorWithVer<T> for Vec<(T, usize)>
where
    T: Clone,
{
    fn project(&self, variable: usize) -> Vec<T> {
        self.iter()
            .filter(|(_, var)| *var == variable)
            .map(|(val, _)| val.clone())
            .collect()
    }
}

pub struct KMPSkipValues<'a> {
    // seen: HashSet<*const NFAHState<'a>>,
    pub skip_values: Vec<HashMap<&'a NFAHState<'a>, usize>>,
}

impl<'a> KMPSkipValues<'a> {
    /// Creates a new `KMPSkipValues` instance.
    ///
    /// # Returns
    ///
    /// A new `KMPSkipValues` instance.
    pub fn new(autom: &'a NFAH<'a>) -> Self {
        // Construct KMP-style skip value
        let mut skip_values = Vec::with_capacity(autom.dimensions);
        for variable in 0..autom.dimensions {
            let states_arena = Arena::new();
            let trans_arena = Arena::new();
            let nfa_states_arena = Arena::new();
            let nfa_trans_arena = Arena::new();
            // The NFA projected to `variable`
            let projected_autom = autom
                .project(&states_arena, &trans_arena, variable)
                .to_nfa_powerset(&nfa_states_arena, &nfa_trans_arena);
            let mut skip_value = HashMap::with_capacity(autom.states.len());
            for loc in autom.iter_states() {
                let autom_loc = autom
                    .project_with_final(loc as *const _, variable, &states_arena, &trans_arena)
                    .to_nfa_powerset(&nfa_states_arena, &nfa_trans_arena);
                let shortest_accepted_word_length = autom_loc.shortest_accepted_word_length();
                for i in 1..shortest_accepted_word_length {
                    let new_inititial_states = autom_loc.states_reachable_in_exactly_n_steps(i);
                    let mut queue = VecDeque::new();
                    let mut seen = HashSet::new();

                    for init_pair in new_inititial_states
                        .iter()
                        .cartesian_product(projected_autom.initial_states.iter())
                    {
                        queue.push_back(init_pair);
                        seen.insert(init_pair);
                    }

                    let mut found_accepting = false;
                    while let Some((left_state, right_state)) = queue.pop_front() {
                        for &left_trans in left_state.transitions.borrow().iter() {
                            let next_left_state = left_trans.next_state;
                            for &right_trans in right_state.transitions.borrow().iter() {
                                if left_trans.label == right_trans.label {
                                    let next_right_state = right_trans.next_state;
                                    // Since for any state, there is at least one reachable finial state, we stop when we arrive at a final state
                                    if next_left_state.is_final || next_right_state.is_final {
                                        found_accepting = true;
                                        skip_value.insert(loc, i);
                                        break;
                                    }
                                    let state_pair =
                                        (&left_trans.next_state, &right_trans.next_state);
                                    if !seen.contains(&state_pair) {
                                        queue.push_back(state_pair);
                                        seen.insert(state_pair);
                                    }
                                }
                            }
                            if found_accepting {
                                break;
                            }
                        }
                        if found_accepting {
                            break;
                        }
                    }
                }
                if !skip_value.contains_key(loc) {
                    skip_value.insert(loc, shortest_accepted_word_length);
                }
            }
            skip_values.push(skip_value);
        }

        KMPSkipValues { skip_values }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::NFAH;
    use typed_arena::Arena;

    #[test]
    fn test_skip_values() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 2);

        let s0 = automaton.add_state(true, false);
        let s1 = automaton.add_state(false, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, false);
        let sf = automaton.add_state(false, true);

        automaton.add_nfah_transition(s0, "c".to_string(), 0, s1);
        automaton.add_nfah_transition(s1, "c".to_string(), 1, s2);
        automaton.add_nfah_transition(s2, "a".to_string(), 0, s3);
        automaton.add_nfah_transition(s3, "b".to_string(), 1, s2);
        automaton.add_nfah_transition(s2, "c".to_string(), 0, sf);

        let kmp_skip_values = KMPSkipValues::new(&automaton);
        assert_eq!(2, kmp_skip_values.skip_values.len());
        assert_eq!(0, kmp_skip_values.skip_values[0][s0]);
        assert_eq!(1, kmp_skip_values.skip_values[0][s1]);
        assert_eq!(1, kmp_skip_values.skip_values[0][s2]);
        assert_eq!(2, kmp_skip_values.skip_values[0][s3]);
        assert_eq!(1, kmp_skip_values.skip_values[0][sf]);
        assert_eq!(0, kmp_skip_values.skip_values[1][s0]);
        assert_eq!(0, kmp_skip_values.skip_values[1][s1]);
        assert_eq!(1, kmp_skip_values.skip_values[1][s2]);
        assert_eq!(1, kmp_skip_values.skip_values[1][s3]);
        assert_eq!(1, kmp_skip_values.skip_values[1][sf]);
    }
}
