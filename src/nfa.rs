use std::collections::{HashMap, HashSet, VecDeque};
use typed_arena::Arena;

use crate::automata::{EpsilonNFA, State, Transition, NFA};

impl<'a> EpsilonNFA<'a> {
    /// Return all states reachable by ε-transitions (None) from any state in `start_set`,
    /// including `start_set` itself. That is, the standard ε-closure for this automaton.
    fn epsilon_closure(
        &self,
        start_set: &HashSet<&'a State<'a, Option<String>>>,
    ) -> HashSet<&'a State<'a, Option<String>>> {
        let mut closure = start_set.clone();
        let mut queue: VecDeque<_> = start_set.iter().copied().collect();

        while let Some(st) = queue.pop_front() {
            // For every None (ε) transition out of st, add that state to closure
            for &trans in st.transitions.borrow().iter() {
                if trans.label.is_none() && !closure.contains(&trans.next_state) {
                    closure.insert(trans.next_state);
                    queue.push_back(trans.next_state);
                }
            }
        }
        closure
    }

    fn gather_alphabet(&self) -> HashSet<String> {
        let mut alphabet = HashSet::new();

        // BFS over all states to find transitions
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        for &initial_state in &self.initial_states {
            queue.push_back(initial_state);
            visited.insert(initial_state as *const _);
        }
        while let Some(st) = queue.pop_front() {
            for &trans in st.transitions.borrow().iter() {
                if let Some(ref sym) = trans.label {
                    alphabet.insert(sym.clone());
                }
                let nxt_ptr = trans.next_state as *const _;
                if !visited.contains(&nxt_ptr) {
                    visited.insert(nxt_ptr);
                    queue.push_back(trans.next_state);
                }
            }
        }
        alphabet
    }

    /// Convert this EpsilonNFA to a plain NFA over `String` via the powerset construction.
    ///
    /// * `new_states_arena`, `new_trans_arena`: typed_arena for the newly constructed NFA.
    ///
    /// Returns a new `NFA<'b>` whose states correspond to subsets of states from `self`.
    /// Transitions carry a `String` label (no epsilons left).
    pub fn to_nfa_powerset<'b>(
        &self,
        new_states_arena: &'b Arena<State<'b, String>>,
        new_trans_arena: &'b Arena<Transition<'b, String>>,
    ) -> NFA<'b> {
        // 1) Collect the full alphabet of "real" symbols (ignoring None).
        let alphabet = self.gather_alphabet();

        // 2) Create the new Automata structure for the resulting NFA
        let mut result = NFA::new(new_states_arena, new_trans_arena, 0);

        // 3) Our "super-states" in the new NFA are sets of old states in the EpsilonNFA.
        //    We'll store them in a canonical manner (for hashing/equality).
        //    A typical approach is to store the pointer addresses in a sorted Vec.
        //    We'll define a small function to produce a stable representation:
        fn canonical_set<'x>(
            states: &HashSet<&'x State<'x, Option<String>>>,
        ) -> Vec<*const State<'x, Option<String>>> {
            let mut addrs: Vec<_> = states
                .iter()
                .map(|s| *s as *const State<'x, Option<String>>)
                .collect::<Vec<*const State<'x, Option<String>>>>();
            addrs.sort();
            addrs
        }

        // 4) We compute the new initial super-state = ε-closure of all initial states.
        let mut initial_set = HashSet::new();
        for &initial_state in &self.initial_states {
            initial_set.insert(initial_state);
        }
        let initial_closure = self.epsilon_closure(&initial_set);

        // The new "super-state" for that closure:
        let init_canonical = canonical_set(&initial_closure);

        // A map from set-of-old-states -> new allocated State
        let mut subset_to_state = HashMap::new();

        // Create the new initial state in the result
        let is_final = initial_closure.iter().any(|st| st.is_final);
        let new_init = result.add_state(true, is_final);
        subset_to_state.insert(init_canonical.clone(), new_init);

        // 5) BFS over the powerset space: from each super-state S, for each symbol in `alphabet`,
        //    gather all destinations and take ε-closure. Then produce a new super-state if needed.
        let mut queue = VecDeque::new();
        queue.push_back(init_canonical);

        while let Some(current_set_repr) = queue.pop_front() {
            let current_state = subset_to_state[&current_set_repr];
            // Reconstruct the set from the canonical representation
            let mut current_set = HashSet::new();
            for ptr in &current_set_repr {
                current_set.insert(unsafe { &*(*ptr) }); // pointer -> reference
            }

            // For each symbol in the alphabet, collect all states reachable by that symbol
            for sym in &alphabet {
                let mut dest_set = HashSet::new();
                // For each old state in `current_set`, follow transitions labeled Some(sym)
                for &old_st in &current_set {
                    for &trans in old_st.transitions.borrow().iter() {
                        if let Some(ref label_str) = trans.label {
                            if label_str == sym {
                                dest_set.insert(trans.next_state);
                            }
                        }
                    }
                }
                // Then take ε-closure of that
                let closure = self.epsilon_closure(&dest_set);
                if closure.is_empty() {
                    // No next super-state => no transition
                    continue;
                }
                let closure_repr = canonical_set(&closure);

                // If we don’t already have a super-state for closure_repr, create one
                let new_super_state = match subset_to_state.get(&closure_repr) {
                    Some(&st) => st,
                    None => {
                        let is_final = closure.iter().any(|st| st.is_final);
                        let st_new = result.add_state(false, is_final);
                        subset_to_state.insert(closure_repr.clone(), st_new);
                        queue.push_back(closure_repr.clone());
                        st_new
                    }
                };

                // Add a transition in the result from current_state to new_super_state
                // labeled by `sym`.
                result.add_transition(current_state, sym.clone(), new_super_state);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use crate::automata::{EpsilonNFA, State};

    #[test]
    fn test_epsilon_nfa_to_nfa_powerset() {
        use std::collections::{HashSet, VecDeque};
        use typed_arena::Arena;

        // 1) Build a small EpsilonNFA
        // States:
        //   s0 (initial), s1 (final), s2 (final)
        // Transitions:
        //   s0 -- None --> s1
        //   s1 -- Some("a") --> s2
        //   s2 -- None --> s0   (cyclic ε)
        let eps_states = Arena::new();
        let eps_trans = Arena::new();
        let mut eps_nfa = EpsilonNFA::new(&eps_states, &eps_trans, 0);

        let s0 = eps_nfa.add_state(true, false);
        let s1 = eps_nfa.add_state(false, true);
        let s2 = eps_nfa.add_state(false, true);

        // s0 -- ε --> s1
        eps_nfa.add_transition(s0, None, s1);
        // s1 -- "a" --> s2
        eps_nfa.add_transition(s1, Some("a".to_string()), s2);
        // s2 -- ε --> s0
        eps_nfa.add_transition(s2, None, s0);

        // 2) Convert to plain NFA
        let nfa_states = Arena::new();
        let nfa_trans = Arena::new();
        let result_nfa = eps_nfa.to_nfa_powerset(&nfa_states, &nfa_trans);

        // 3) Examine the resulting NFA
        // We'll BFS from the new initial states and collect transitions for inspection.
        assert_eq!(result_nfa.initial_states.len(), 1);
        let new_init = result_nfa.initial_states[0];

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        queue.push_back(new_init);
        visited.insert(new_init as *const _);

        let mut transitions_info = Vec::new();

        while let Some(st) = queue.pop_front() {
            for &tr in st.transitions.borrow().iter() {
                // record (from, label, to, is_final_of_to)
                transitions_info.push((
                    format!("{:p}", st),
                    tr.label.clone(),
                    format!("{:p}", tr.next_state),
                    tr.next_state.is_final,
                ));
                let nxt_ptr = tr.next_state as *const State<String>;
                if !visited.contains(&nxt_ptr) {
                    visited.insert(nxt_ptr);
                    queue.push_back(tr.next_state);
                }
            }
        }

        // We expect to see transitions labeled "a" in the new NFA, but no None transitions.
        // We also expect at least one final state in the new NFA because the original had final states.

        // Let's just check that there's at least one transition with label "a"
        // (the original E-NFA had "a" from s1 -> s2).
        let has_a_transition = transitions_info
            .iter()
            .any(|(_from, lbl, _to, _fin)| lbl == "a");
        assert!(
            has_a_transition,
            "Expected at least one transition labeled \"a\" in the powerset NFA."
        );

        // Check that there's at least one final state in the new NFA
        let any_final_state = visited.iter().any(|&ptr| unsafe { &*ptr }.is_final);
        assert!(
            any_final_state,
            "Expected at least one final state in the powerset-constructed NFA."
        );

        // For debugging, you could print out the transitions_info
        // println!("Transitions in powerset NFA: {:#?}", transitions_info);
    }
}
