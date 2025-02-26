use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};

use itertools::Itertools;
use typed_arena::Arena;

use crate::automata::{Automata, State, Transition, ValidLabel, NFA};

/// A Deterministic Finite Automaton (DFA) over alphabet `A` with states of type `S`.
#[derive(Debug, Clone)]
pub struct DFA<S, A> {
    /// All states in the DFA.
    pub states: HashSet<S>,
    /// The input alphabet symbols recognized by this DFA.
    pub alphabet: HashSet<A>,
    /// The unique initial state.
    pub initial: S,
    /// A set of accepting (final) states.
    pub finals: HashSet<S>,
    /// The transition function: given (current_state, symbol) → next_state.
    /// Must be deterministic, so there should be at most one outcome for each pair.
    pub transitions: HashMap<(S, A), S>,
}

impl<S, A> DFA<S, A>
where
    S: Eq + Hash + Clone,
    A: Eq + Hash + Clone + Debug,
{
    /// Creates a new DFA, specifying:
    /// - The initial state
    /// - The DFA alphabet
    ///
    /// Initially, the DFA has only the initial state, which is not final. You can add
    /// more states, transitions, and final states as needed.
    pub fn new(initial: S, alphabet: HashSet<A>) -> Self {
        let mut states = HashSet::new();
        states.insert(initial.clone());

        DFA {
            states,
            alphabet,
            initial,
            finals: HashSet::new(),
            transitions: HashMap::new(),
        }
    }

    /// Adds a new state to the DFA. By default, it's not marked as final.
    /// Returns `true` if the state was newly inserted, or `false` if it already existed.
    pub fn add_state(&mut self, s: S) -> bool {
        self.states.insert(s)
    }

    /// Marks the given state as an accepting (final) state.
    /// If the state does not exist yet, we insert it as well.
    pub fn set_final(&mut self, s: S) {
        self.states.insert(s.clone());
        self.finals.insert(s);
    }

    /// Adds a transition to the DFA: from state `from` on symbol `sym` to state `to`.
    /// If `from` or `to` do not exist in the DFA, we add them automatically.
    /// Panics if `sym` is not in the alphabet.
    pub fn add_transition(&mut self, from: S, sym: A, to: S) {
        if !self.alphabet.contains(&sym) {
            panic!("Symbol {:?} not in DFA alphabet!", sym);
        }

        // Ensure states are present
        self.states.insert(from.clone());
        self.states.insert(to.clone());

        // Insert deterministic transition
        // Overwriting an existing transition is possible if you want to redefine
        // but typically you'd check for duplicates if that’s disallowed.
        self.transitions.insert((from, sym), to);
    }

    /// Tests whether the DFA accepts the given input word.
    pub fn accepts(&self, input: &[A]) -> bool {
        // Start at the initial state
        let mut current_state = self.initial.clone();

        // Consume the input
        for sym in input {
            // If no transition is defined, the DFA rejects
            match self.transitions.get(&(current_state.clone(), sym.clone())) {
                Some(next_st) => {
                    current_state = next_st.clone();
                }
                None => {
                    return false; // no valid transition => reject
                }
            }
        }

        // After consuming the entire word, check if we're in a final state
        self.finals.contains(&current_state)
    }
}

/// A simple wrapper around `HashSet<S>` that implements `Hash` in a canonical way.
/// We do this so we can store `StateSet<S>` as keys in a HashMap/HashSet.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct StateSet<S: Hash + Eq>(pub HashSet<S>);

impl<S: Eq + Hash> Hash for StateSet<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Strategy: create a list of per-element hashes, sort them, then feed them into `state`.
        // This ensures that the iteration order does not affect the final hash.
        let mut element_hashes: Vec<u64> = self
            .0
            .iter()
            .map(|elem| {
                let mut hasher = DefaultHasher::new();
                elem.hash(&mut hasher);
                hasher.finish()
            })
            .collect();

        element_hashes.sort_unstable();
        for h in element_hashes {
            h.hash(state);
        }
    }
}

/// A reversed version of a DFA can be considered an NFA:
///   - `initials` are the old finals
///   - `finals` is the old initial
///   - transitions are reversed: for each (p, a) -> q in the DFA,
///     we have (q, a) -> p in the reversed NFA.
#[derive(Debug, Clone)]
struct ReversedNFA<S, A> {
    states: HashSet<S>,
    alphabet: HashSet<A>,
    initials: HashSet<S>,
    finals: HashSet<S>,
    transitions: HashMap<(S, A), HashSet<S>>,
}

impl<S, A> DFA<S, A>
where
    S: Eq + Hash + Clone,
    A: Eq + Hash + Clone + ValidLabel + Debug,
{
    /// Build an NFA that is the reverse of this DFA:
    ///   - new initial states = old finals
    ///   - new final states = { old initial }
    ///   - for each (p, a)->q in the DFA, we add (q, a)->p in the NFA.
    fn to_reversed_nfa<'a>(
        &self,
        states: &'a Arena<State<'a, A>>,
        transitions: &'a Arena<Transition<'a, A>>,
    ) -> Automata<'a, A> {
        let mut nfa = Automata::new(states, transitions, 1);
        let mut old_to_new = HashMap::new();
        for state in self.states.clone() {
            let is_final = self.initial == state;
            let is_initial = self.finals.contains(&state);
            old_to_new.insert(state, nfa.add_state(is_initial, is_final));
        }

        // Reverse each transition
        for ((from, label), to) in &self.transitions {
            if let Some(new_from) = old_to_new.get(to) {
                if let Some(new_to) = old_to_new.get(from) {
                    nfa.add_transition(new_from, label.clone(), new_to);
                }
            }
        }

        nfa
    }
}

impl<'a, L> Automata<'a, L>
where
    L: Eq + Hash + Clone + ValidLabel + Debug,
{
    fn determinize(&self) -> DFA<usize, L> {
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
                alphabet.insert(trans.label.clone());
                let nxt_ptr = trans.next_state as *const _;
                if !visited.contains(&nxt_ptr) {
                    visited.insert(nxt_ptr);
                    queue.push_back(trans.next_state);
                }
            }
        }

        // 1) Build the new DFA with initial subset
        let init_subset: StateSet<&State<'a, L>> =
            StateSet(self.initial_states.iter().cloned().collect());
        let mut states = HashMap::new();
        states.insert(init_subset.clone(), 0);
        let mut dfa = DFA::new(0, alphabet.clone().into_iter().collect());

        // check finals
        if init_subset.0.iter().any(|state| state.is_final) {
            dfa.set_final(*states.get(&init_subset.clone()).unwrap());
        }

        // BFS
        let mut visited = std::collections::HashSet::new();
        visited.insert(init_subset.clone());
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(init_subset);

        while let Some(current_subset) = queue.pop_front() {
            for sym in &alphabet {
                let mut next_set = HashSet::new();
                // gather transitions
                for state in &current_subset.0 {
                    for transition in state.transitions.borrow().iter() {
                        if transition.label == *sym {
                            next_set.insert(transition.next_state);
                        }
                    }
                }
                if next_set.is_empty() {
                    continue;
                }
                let next_subset = StateSet(next_set);
                if !visited.contains(&next_subset) {
                    visited.insert(next_subset.clone());
                    states.insert(next_subset.clone(), states.len());
                    if next_subset.0.iter().any(|state| state.is_final) {
                        dfa.set_final(*states.get(&next_subset).unwrap());
                    }
                    queue.push_back(next_subset.clone());
                }
                dfa.add_transition(
                    *states.get(&current_subset).unwrap(),
                    sym.clone(),
                    *states.get(&next_subset).unwrap(),
                );
            }
        }

        dfa
    }
}

impl<S, A> DFA<S, A>
where
    S: Eq + Hash + Clone,
    A: Eq + Hash + Clone + ValidLabel + Debug,
{
    /// Minimizes this DFA using Brzozowski's algorithm.
    ///
    /// 1) Reverse (to NFA)
    /// 2) Determinize -> dfa1
    /// 3) Reverse dfa1 (to NFA)
    /// 4) Determinize -> final minimal dfa
    pub fn minimize_brzozowski<'a>(
        &self,
        state_arena: &'a Arena<State<'a, A>>,
        trans_arena: &'a Arena<Transition<'a, A>>,
    ) -> DFA<usize, A> {
        // Step 1: Reverse original -> NFA
        let rev_nfa = self.to_reversed_nfa(state_arena, trans_arena);
        // Step 2: Determinize -> intermediate DFA
        let dfa1 = rev_nfa.determinize();

        // // Step 3: Reverse dfa1 -> NFA
        let rev_nfa2 = dfa1.to_reversed_nfa(state_arena, trans_arena);

        // // Step 4: Determinize -> minimal DFA
        rev_nfa2.determinize()
    }
}

// Remove this entire block

impl<S, A> DFA<S, A>
where
    S: Eq + Hash + Clone + Debug,
    A: Eq + Hash + Clone + Debug,
{
    /// Negates (complements) this DFA, assuming it is complete.
    /// The resulting DFA accepts exactly those words that the original rejects.
    pub fn negate(&self) -> DFA<S, A> {
        // 1) Optional check that the DFA is "complete".
        for s in &self.states {
            for sym in &self.alphabet {
                if !self.transitions.contains_key(&(s.clone(), sym.clone())) {
                    panic!(
                        "DFA is not complete, missing transition from {:?} on {:?}",
                        s, sym
                    );
                }
            }
        }

        // 2) Clone everything except the final states
        let mut new_dfa = DFA {
            states: self.states.clone(),
            alphabet: self.alphabet.clone(),
            initial: self.initial.clone(),
            finals: HashSet::new(),
            transitions: self.transitions.clone(),
        };

        // 3) Flip final vs. non-final
        //    i.e., new_final = states - old_final
        for s in &self.states {
            if !self.finals.contains(s) {
                new_dfa.finals.insert(s.clone());
            }
        }

        new_dfa
    }

    /// Make this DFA complete by adding a "sink" state (if needed).
    /// We name the sink state using a provided function or by generating a fresh label.
    /// Then, for every missing transition (s, a), define s--a--> sink.
    /// And make every transition from sink loop back to sink.
    pub fn make_complete(&mut self, sink_label: S) {
        if self.states.contains(&sink_label) {
            // your code to handle collision, or panic, or rename
        } else {
            // Insert the sink state
            self.states.insert(sink_label.clone());
        }

        // For each state s, for each symbol a in the alphabet,
        // if no transition defined, add s--a--> sink_label
        for s in self.states.clone() {
            for sym in &self.alphabet {
                let key = (s.clone(), sym.clone());
                self.transitions
                    .entry(key)
                    .or_insert_with(|| sink_label.clone());
            }
        }

        // Also, from sink state we ensure we remain in sink on every symbol
        for sym in &self.alphabet {
            let key = (sink_label.clone(), sym.clone());
            self.transitions.insert(key, sink_label.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_dfa_accepts_ends_in_one() {
        // We'll define states as simple integers: 0 => "last bit was 0", 1 => "last bit was 1".
        // But conceptually, the "meaning" is "the last bit we read was 0 or 1"?
        // Actually, we can do "0 => ended in 0, 1 => ended in 1".
        //
        // We'll accept binary strings that end in '1'. So state "1" is final, "0" is not final.

        let mut alphabet = HashSet::new();
        alphabet.insert('0');
        alphabet.insert('1');

        // Create a new DFA, with initial state = 0, alphabet = { '0', '1' }.
        let mut dfa = DFA::new(0, alphabet);

        // Add the second state, which we'll call "1".
        dfa.add_state(1);

        // Mark "1" as a final state
        dfa.set_final(1);

        // Add transitions:
        // from state 0 on '0' -> 0, on '1' -> 1
        dfa.add_transition(0, '0', 0);
        dfa.add_transition(0, '1', 1);

        // from state 1 on '0' -> 0, on '1' -> 1
        dfa.add_transition(1, '0', 0);
        dfa.add_transition(1, '1', 1);

        // Now let's test some strings
        // "" (empty) => ends in 0 by default? Actually, we start in state 0, which is not final => reject
        assert!(!dfa.accepts(&[]));

        // "1" => state transitions: 0 --'1'--> 1 => final => accept
        assert!(dfa.accepts(&['1']));

        // "0" => 0 --'0'--> 0 => not final => reject
        assert!(!dfa.accepts(&['0']));

        // "10110" => let's see:
        //   start: 0
        //   read '1': 0->1
        //   read '0': 1->0
        //   read '1': 0->1
        //   read '1': 1->1
        //   read '0': 1->0
        // end in state 0 => not final => reject
        assert!(!dfa.accepts(&['1', '0', '1', '1', '0']));

        // "10111" => ends with '1'
        //  same steps, but last input is '1':
        //   1->1 -> we remain in state 1 => final => accept
        assert!(dfa.accepts(&['1', '0', '1', '1', '1']));
    }

    #[test]
    fn test_brzozowski_minimization() {
        // We'll define a DFA over {0,1} that accepts
        // all binary strings containing "11" as a substring.
        //
        // States (conceptual):
        //  S0 = have seen nothing or no '1' last
        //  S1 = last bit was '1' but haven't seen "11" yet
        //  S2 = have seen "11" (accepting)
        //
        // Transitions:
        //  S0 --'0'--> S0
        //  S0 --'1'--> S1
        //  S1 --'0'--> S0
        //  S1 --'1'--> S2
        //  S2 --'0'--> S2  (once we've seen "11", we stay accepted)
        //  S2 --'1'--> S2

        let mut sigma = HashSet::new();
        sigma.insert('0');
        sigma.insert('1');

        let mut dfa = DFA::new("S0".to_string(), sigma);

        dfa.add_state("S1".to_string());
        dfa.add_state("S2".to_string());

        dfa.set_final("S2".to_string());

        dfa.add_transition("S0".to_string(), '0', "S0".to_string());
        dfa.add_transition("S0".to_string(), '1', "S1".to_string());
        dfa.add_transition("S1".to_string(), '0', "S0".to_string());
        dfa.add_transition("S1".to_string(), '1', "S2".to_string());
        dfa.add_transition("S2".to_string(), '0', "S2".to_string());
        dfa.add_transition("S2".to_string(), '1', "S2".to_string());

        // Check a couple of examples
        assert!(!dfa.accepts(&['0', '0', '1', '0'])); // no "11"
        assert!(dfa.accepts(&['1', '1'])); // "11" found
        assert!(dfa.accepts(&['1', '0', '1', '1', '0'])); // "11" found
        assert!(!dfa.accepts(&['0', '1', '0', '1', '0'])); // no "11"

        // Minimization with Brzozowski:
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let minimized = dfa.minimize_brzozowski(&state_arena, &trans_arena);

        // The minimized machine should still accept "11" and only that pattern,
        // but typically with fewer (or same) states if any were redundant.
        assert!(!minimized.accepts(&['0', '0', '1', '0']));
        assert!(minimized.accepts(&['1', '1']));
        assert!(minimized.accepts(&['1', '0', '1', '1', '0']));
        assert!(!minimized.accepts(&['0', '1', '0', '1', '0']));

        // You can optionally print or debug the minimized DFA states
        // println!("Minimized states: {:?}", minimized.states);
        // println!("Minimized transitions: {:?}", minimized.transitions);
        // Typically it might produce 3 states anyway for this language, or fewer if merges are possible.
    }

    #[test]
    fn test_dfa_negation() {
        // We'll define a complete DFA for "ends in 1"
        let mut sigma = HashSet::new();
        sigma.insert('0');
        sigma.insert('1');

        // initial = 0 => "ends in 0"
        // we also have state 1 => "ends in 1"
        let mut dfa = DFA::new(0, sigma);

        dfa.add_state(1);
        dfa.set_final(1);

        // transitions (complete):
        //   0 --'0'--> 0
        //   0 --'1'--> 1
        //   1 --'0'--> 0
        //   1 --'1'--> 1
        dfa.add_transition(0, '0', 0);
        dfa.add_transition(0, '1', 1);
        dfa.add_transition(1, '0', 0);
        dfa.add_transition(1, '1', 1);

        // quick checks
        assert!(!dfa.accepts(&[])); // empty => state=0 => not final
        assert!(dfa.accepts(&['1']));
        assert!(dfa.accepts(&['0', '1']));
        assert!(!dfa.accepts(&['1', '0', '1', '0'])); // ends in 0

        // Negate it
        let neg_dfa = dfa.negate();

        // Now everything is flipped
        assert!(neg_dfa.accepts(&[])); // original was false
        assert!(!neg_dfa.accepts(&['1']));
        assert!(!neg_dfa.accepts(&['0', '1']));
        assert!(neg_dfa.accepts(&['1', '0', '1', '0']));
    }
}

#[test]
fn test_dfa_negation() {
    // We'll define a complete DFA for "ends in 1"
    let mut sigma = HashSet::new();
    sigma.insert('0');
    sigma.insert('1');

    // initial = 0 => "ends in 0"
    // we also have state 1 => "ends in 1"
    let mut dfa = DFA::new(0, sigma);

    dfa.add_state(1);
    dfa.set_final(1);

    // transitions (complete):
    //   0 --'0'--> 0
    //   0 --'1'--> 1
    //   1 --'0'--> 0
    //   1 --'1'--> 1
    dfa.add_transition(0, '0', 0);
    dfa.add_transition(0, '1', 1);
    dfa.add_transition(1, '0', 0);
    dfa.add_transition(1, '1', 1);

    // quick checks
    assert!(!dfa.accepts(&[])); // empty => state=0 => not final
    assert!(dfa.accepts(&['1']));
    assert!(dfa.accepts(&['0', '1']));
    assert!(!dfa.accepts(&['1', '0', '1', '0'])); // ends in 0

    // Negate it
    let neg_dfa = dfa.negate();

    // Now everything is flipped
    assert!(neg_dfa.accepts(&[])); // original was false
    assert!(!neg_dfa.accepts(&['1']));
    assert!(!neg_dfa.accepts(&['0', '1']));
    assert!(neg_dfa.accepts(&['1', '0', '1', '0']));
}
