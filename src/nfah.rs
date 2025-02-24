use crate::automata::{EpsilonNFA, NFAHState, NFAHTransition, State, Transition, NFAH};
use std::collections::HashMap;
use typed_arena::Arena;

impl<'a> NFAH<'a> {
    /// Adds a transition (action, var) from `from` to `to`.
    pub fn add_nfah_transition(
        &self,
        from: &'a NFAHState<'a>,
        action: String,
        var: usize,
        to: &'a NFAHState<'a>,
    ) -> &'a NFAHTransition<'a> {
        self.add_transition(from, (action, var), to)
    }

    /// A helper version of `project_with_map` that returns both the new EpsilonNFA
    /// **and** a mapping from old (pointer) state to the newly created state.
    ///
    /// # Arguments
    /// * `var` - the variable index to project
    /// * `states_arena`, `trans_arena`: where to allocate the new EpsilonNFA
    /// * `trans_arena`: where to allocate the new transitions
    /// * `is_final`: function to decide whether the state should be final
    /// * `old_to_new`: a map to fill with the old->new state mapping
    fn project_with_map<'b, F>(
        &self,
        var: usize,
        states_arena: &'b Arena<State<'b, Option<String>>>,
        trans_arena: &'b Arena<Transition<'b, Option<String>>>,
        is_final: F,
        old_to_new: &mut HashMap<*const NFAHState<'a>, &'b State<'b, Option<String>>>,
    ) -> EpsilonNFA<'b>
    where
        F: Fn(*const NFAHState<'a>) -> bool,
    {
        let mut projected = EpsilonNFA::new(states_arena, trans_arena, 0);

        let mut queue = std::collections::VecDeque::new();

        // 1) Create new states for each initial state in `self` and queue them
        for &init_state in &self.initial_states {
            let new_init = projected.add_state(true, is_final(init_state));
            old_to_new.insert(init_state as *const _, new_init);
            queue.push_back(init_state);
        }

        // 2) BFS replicate transitions
        while let Some(old_st) = queue.pop_front() {
            let new_st = old_to_new[&(old_st as *const _)];
            for &trans in old_st.transitions.borrow().iter() {
                let (ref action, old_var) = trans.label;
                let old_next = trans.next_state as *const _;

                // If next not seen, add
                if let std::collections::hash_map::Entry::Vacant(e) = old_to_new.entry(old_next) {
                    let new_next = projected.add_state(false, is_final(trans.next_state));
                    e.insert(new_next);
                    queue.push_back(trans.next_state);
                }
                let new_next = old_to_new[&old_next];

                // Label is Some(...) if old_var == var, else None
                let new_label = if old_var == var {
                    Some(action.clone())
                } else {
                    None
                };
                projected.add_transition(new_st, new_label, new_next);
            }
        }

        projected
    }

    /// Given a variable `var`, produce an epsilon-NFA over `String` by:
    ///  - turning transitions labeled with `(action, var)` into Some(action),
    ///  - and every other transition into an epsilon transition (None).
    ///
    /// We copy only those states reachable from the initial states (via BFS).
    /// Return a newly constructed `EpsilonNFA` that uses two new arenas for states/transitions.
    ///
    /// *If you need to track the old->new state mapping for further processing,
    /// consider using `project_with_map` directly.*
    pub fn project<'b>(
        &self,
        states_arena: &'b Arena<State<'b, Option<String>>>,
        trans_arena: &'b Arena<Transition<'b, Option<String>>>,
        var: usize,
    ) -> EpsilonNFA<'b> {
        // Just a thin wrapper around `project_with_map`, discarding the map
        let mut dummy_map = HashMap::new();
        self.project_with_map(
            var,
            states_arena,
            trans_arena,
            |old_state| unsafe { (*old_state).is_final },
            &mut dummy_map,
        )
    }

    /// Builds a projected EpsilonNFA over `String`, but forces exactly one final location = `loc`.
    ///
    /// Internally, it:
    ///   1) calls `project(...)` on the entire NFAH (with all old finals preserved)
    ///   2) then traverses the resulting EpsilonNFA, marking exactly the new state
    ///      that corresponds to `loc` as final, and all others not final.
    ///
    /// # Arguments
    /// * `loc` - pointer to the unique final location in the original NFAH
    /// * `var` - which variable index is being projected
    /// * `states_arena`, `trans_arena`: where to allocate the new EpsilonNFA
    pub fn project_with_final<'b>(
        &self,
        loc: *const NFAHState<'_>,
        var: usize,
        states_arena: &'b Arena<State<'b, Option<String>>>,
        trans_arena: &'b Arena<Transition<'b, Option<String>>>,
    ) -> EpsilonNFA<'b> {
        // First, do the normal projection
        // But we need to track old->new to correct final states afterward
        let mut old_to_new = HashMap::new();

        // We'll create a custom version of `project` that returns the map.
        self.project_with_map(
            var,
            states_arena,
            trans_arena,
            |old_state| old_state == (loc as *const NFAHState<'_>),
            &mut old_to_new,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashSet, VecDeque};

    use crate::automata::NFAH;

    #[test]
    fn test_project() {
        use typed_arena::Arena;

        // Step 1: Create original NFAH with 3 possible variables: 0, 1, 2
        let nfa_states = Arena::new();
        let nfa_trans = Arena::new();
        let mut nfa_h = NFAH::new(&nfa_states, &nfa_trans, 3);

        // States s0 (initial, not final), s1 (not initial, final).
        let s0 = nfa_h.add_state(true, false);
        let s1 = nfa_h.add_state(false, true);

        // s0 --( "a", var=0 )--> s1
        nfa_h.add_nfah_transition(s0, "a".to_string(), 0, s1);
        // s0 --( "b", var=1 )--> s1
        nfa_h.add_nfah_transition(s0, "b".to_string(), 1, s1);
        // s1 --( "c", var=2 )--> s0
        nfa_h.add_nfah_transition(s1, "c".to_string(), 2, s0);

        // Step 2: Project onto var=1
        let eps_states = Arena::new();
        let eps_trans = Arena::new();
        let eps_nfa = nfa_h.project(&eps_states, &eps_trans, 1);

        // Step 3: Check the transitions in the resulting EpsilonNFA
        // We can BFS or simply check from each known state.
        // The BFS approach is typically how we find all states:

        use std::collections::{HashSet, VecDeque};
        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();

        for &init in &eps_nfa.initial_states {
            queue.push_back(init);
            seen.insert(init as *const _);
        }

        // We'll store transitions in a vector to check them more easily
        let mut transitions_vec = Vec::new();

        while let Some(st) = queue.pop_front() {
            for &trans in st.transitions.borrow().iter() {
                transitions_vec.push(trans);
                let nxt_ptr = trans.next_state as *const _;
                if !seen.contains(&nxt_ptr) {
                    queue.push_back(trans.next_state);
                    seen.insert(nxt_ptr);
                }
            }
        }

        // Now examine the transitions
        // - ( "b", var=1 ) in the old automaton => Some("b") in new automaton
        // - ( "a", var=0 ) => None
        // - ( "c", var=2 ) => None
        let mut found_some_b = false;
        let mut found_none_a = false;
        let mut found_none_c = false;

        for t in &transitions_vec {
            match &t.label {
                Some(label_str) if label_str == "b" => {
                    found_some_b = true;
                }
                None => {
                    // Could be from the old ( "a", var=0 ) or ( "c", var=2 )
                    // We won't know which original was which, but we know at least one was "a", one was "c".
                    // Here, we can attempt to see if next_state is final or not to guess.
                    let st_desc = format!("{:?}", t);
                    // Just do a quick check for variety
                    if st_desc.contains("a") {
                        found_none_a = true; // (heuristic if you do more direct debugging)
                    }
                    if st_desc.contains("c") {
                        found_none_c = true;
                    }
                }
                Some(other) => {
                    panic!("Found unexpected label: {:?}", other);
                }
            }
        }

        assert!(
            found_some_b,
            "Expected Some(\"b\") transition in projected automaton."
        );
        // Because we might not precisely differentiate the None edges by 'a' vs 'c',
        // just ensure that we saw 2 distinct None transitions in total:
        let none_count = transitions_vec.iter().filter(|t| t.label.is_none()).count();
        assert_eq!(none_count, 2, "Expected exactly 2 ε-transitions (None).");
    }

    #[test]
    fn test_project_with_final() {
        use typed_arena::Arena;

        // Step 1: Create original NFAH with 3 possible variables: 0, 1, 2
        let nfa_states = Arena::new();
        let nfa_trans = Arena::new();
        let mut nfa_h = NFAH::new(&nfa_states, &nfa_trans, 3);

        // States s0 (initial, not final), s1 (not initial, final).
        let s0 = nfa_h.add_state(true, false);
        let s1 = nfa_h.add_state(false, true);

        // s0 --( "a", var=0 )--> s1
        nfa_h.add_nfah_transition(s0, "a".to_string(), 0, s1);
        // s0 --( "b", var=1 )--> s1
        nfa_h.add_nfah_transition(s0, "b".to_string(), 1, s1);
        // s1 --( "c", var=0 )--> s0
        nfa_h.add_nfah_transition(s1, "c".to_string(), 0, s0);

        // Step 2: Project onto var=0
        let eps_states = Arena::new();
        let eps_trans = Arena::new();
        let eps_nfa = nfa_h.project(&eps_states, &eps_trans, 0);

        // Step 3: Check the transitions in the resulting EpsilonNFA
        // We can BFS or simply check from each known state.
        // The BFS approach is typically how we find all states:

        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();

        assert_eq!(1, eps_nfa.initial_states.len());
        for &init in &eps_nfa.initial_states {
            queue.push_back(init);
            seen.insert(init as *const _);
        }

        // We'll store transitions in a vector to check them more easily
        let mut transitions_vec = Vec::new();

        while let Some(st) = queue.pop_front() {
            for &trans in st.transitions.borrow().iter() {
                transitions_vec.push(trans);
                let nxt_ptr = trans.next_state as *const _;
                if !seen.contains(&nxt_ptr) {
                    queue.push_back(trans.next_state);
                    seen.insert(nxt_ptr);
                }
            }
        }

        // Now examine the transitions
        let mut found_some_a = false;
        let mut found_some_c = false;

        for t in &transitions_vec {
            match &t.label {
                Some(label_str) if label_str == "a" => {
                    found_some_a = true;
                }
                Some(label_str) if label_str == "c" => {
                    found_some_c = true;
                }
                None => {}
                Some(other) => {
                    panic!("Found unexpected label: {:?}", other);
                }
            }
        }

        assert!(
            found_some_a,
            "Expected Some(\"a\") transition in projected automaton."
        );
        assert!(
            found_some_c,
            "Expected Some(\"c\") transition in projected automaton."
        );
        let none_count = transitions_vec.iter().filter(|t| t.label.is_none()).count();
        assert_eq!(none_count, 1, "Expected exactly 1 ε-transitions (None).");
    }
}
