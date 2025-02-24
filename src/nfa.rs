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

impl<'a> NFA<'a> {
    /// Builds the (intersection) product automaton of `self` and `other`.
    ///
    /// Both NFAs must be over the same alphabet type (`String`).
    /// The resulting automaton is also an `NFA<'b>` using new arenas.
    ///
    /// Intersection semantics:
    ///  - The product state is final if both component states are final.
    ///  - A transition with label ℓ from (s1, s2) to (t1, t2) exists if
    ///    there is a transition labeled ℓ from s1 -> t1 in `self`
    ///    **and** a transition labeled ℓ from s2 -> t2 in `other`.
    pub fn product<'b>(
        &self,
        other: &NFA<'a>,
        new_states_arena: &'b Arena<State<'b, String>>,
        new_trans_arena: &'b Arena<Transition<'b, String>>,
    ) -> NFA<'b> {
        // Create the new NFA (product automaton).
        // Dimension can be 0 because we are not using "var" indexing here.
        let mut product_nfa = NFA::new(new_states_arena, new_trans_arena, 0);

        // We'll map (s1_ptr, s2_ptr) -> newly created product state
        let mut pair_to_state = HashMap::new();
        let mut queue = VecDeque::new();

        // 1) Create product initial states from all pairs of (init1, init2)
        for &init1 in &self.initial_states {
            for &init2 in &other.initial_states {
                let is_final = init1.is_final && init2.is_final;
                let prod_init = product_nfa.add_state(true, is_final);
                pair_to_state.insert((init1 as *const _, init2 as *const _), prod_init);
                queue.push_back((init1, init2));
            }
        }

        // 2) BFS in the space of (s1, s2) pairs
        while let Some((old_s1, old_s2)) = queue.pop_front() {
            // The product state we already created:
            let new_current = pair_to_state[&(old_s1 as *const _, old_s2 as *const _)];

            // We'll gather transitions by label for each side
            let mut transitions_1: HashMap<&str, Vec<&State<'_, String>>> = HashMap::new();
            for &t1 in old_s1.transitions.borrow().iter() {
                transitions_1
                    .entry(&t1.label)
                    .or_default()
                    .push(t1.next_state);
            }

            let mut transitions_2: HashMap<&str, Vec<&State<'_, String>>> = HashMap::new();
            for &t2 in old_s2.transitions.borrow().iter() {
                transitions_2
                    .entry(&t2.label)
                    .or_default()
                    .push(t2.next_state);
            }

            // For each label that appears in both transition sets, cross-product
            // all possible next states
            for (lbl, nexts1) in &transitions_1 {
                if let Some(nexts2) = transitions_2.get(lbl) {
                    // For each possible next pair (n1, n2)
                    for &n1 in nexts1 {
                        for &n2 in nexts2 {
                            let key = (n1 as *const _, n2 as *const _);
                            let new_next = match pair_to_state.get(&key) {
                                Some(&existing) => existing,
                                None => {
                                    let is_fin = n1.is_final && n2.is_final;
                                    let st_new = product_nfa.add_state(false, is_fin);
                                    pair_to_state.insert(key, st_new);
                                    queue.push_back((n1, n2));
                                    st_new
                                }
                            };
                            // Add the transition in the product
                            product_nfa.add_transition(new_current, (*lbl).to_string(), new_next);
                        }
                    }
                }
            }
        }

        product_nfa
    }
}

#[cfg(test)]
mod tests {
    use crate::automata::{EpsilonNFA, State};
    use crate::nfa::NFA;

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

    #[test]
    fn test_product_nfa() {
        use typed_arena::Arena;

        // Build first NFA: accepts "ab" or "aab" (small example)
        //
        //  States: s0 (initial), s1, s2 (final)
        //  Transitions:
        //    s0 --"a"--> s1
        //    s1 --"b"--> s2
        //    s0 --"a"--> s0  (so it can read multiple 'a's before a single 'b'?)
        //  => Accepts strings of one or more 'a' followed by a single 'b' ("ab", "aab", "aaab", ...)
        let arena_s1 = Arena::new();
        let arena_t1 = Arena::new();
        let mut nfa1 = NFA::new(&arena_s1, &arena_t1, 0);

        let s0_1 = nfa1.add_state(true, false);
        let s1_1 = nfa1.add_state(false, false);
        let s2_1 = nfa1.add_state(false, true); // final

        // transitions
        nfa1.add_transition(s0_1, "a".to_string(), s1_1);
        nfa1.add_transition(s1_1, "b".to_string(), s2_1);
        // optional "loop on a" at s0_1 if we want multiple a's
        nfa1.add_transition(s0_1, "a".to_string(), s0_1);

        // Build second NFA: accepts "aab" or "abc"
        //
        //   States: p0 (initial), p1, p2 (final), p3 (final)
        //   Transitions:
        //       p0 --"a"--> p0
        //       p0 --"a"--> p1
        //       p1 --"b"--> p2 (final)
        //       p1 --"b"--> p3 (final) but then p3 --"c"--> ??? Actually let's do:
        //         p1 --"b"--> p3
        //         p3 --"c"--> p2 (final). So "abc" is recognized if we do p0->p1->p3->p2 with "a","b","c"?
        //   => Accepts infinite set that always starts with >=1 'a',
        //      but has 2 final paths:  "ab" (p1->p2) or "abc" (p1->p3->p2).
        let arena_s2 = Arena::new();
        let arena_t2 = Arena::new();
        let mut nfa2 = NFA::new(&arena_s2, &arena_t2, 0);

        let p0_2 = nfa2.add_state(true, false);
        let p1_2 = nfa2.add_state(false, false);
        let p2_2 = nfa2.add_state(false, true); // final
        let p3_2 = nfa2.add_state(false, false);

        // transitions
        nfa2.add_transition(p0_2, "a".to_string(), p0_2);
        nfa2.add_transition(p0_2, "a".to_string(), p1_2);
        nfa2.add_transition(p1_2, "b".to_string(), p2_2);
        nfa2.add_transition(p1_2, "b".to_string(), p3_2);
        // let p3 lead to p2 on "c"
        nfa2.add_transition(p3_2, "c".to_string(), p2_2);

        // The second NFA accepts strings like "ab", "aab", "aaab", plus "abc", "aabc", etc.

        // 3) Build the product
        let product_s_arena = Arena::new();
        let product_t_arena = Arena::new();
        let product_nfa = nfa1.product(&nfa2, &product_s_arena, &product_t_arena);

        // 4) Check if the product is empty or not
        // Because both accept strings that start with >=1 'a' and end with 'b',
        // they definitely share some accepted strings. For example, "ab" is accepted by both:
        //   - NFA1: s0->s1->s2 with "a","b"  (or s0->s0->s0->...->s1->s2 if multiple 'a's)
        //   - NFA2: p0->p1->p2 with "a","b"
        // => The product language is definitely non-empty
        assert!(
            !product_nfa.is_empty(),
            "They both accept 'ab', so intersection is not empty."
        );

        // (Optional) We can do more checks, e.g. shortest_accepted_word_length
        let length = product_nfa.shortest_accepted_word_length();
        assert_eq!(
            length, 2,
            "Shortest word in the intersection is 'ab' of length 2."
        );

        // For more debugging, you could BFS over the product states and print them:
        // println!("product automaton states: ...");
    }
}
