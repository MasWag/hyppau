use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use typed_arena::Arena;

/// Represents a transition between states in an automaton.
///
/// A transition includes an action, which is a k-tuple of strings that triggers the transition and a reference to the next state.
#[derive(Debug, PartialEq)]
pub struct Transition<'a> {
    /// The action that triggers this transition.
    pub action: Vec<String>,
    /// The state to transition to.
    pub next_state: &'a State<'a>,
}

impl<'a> Hash for Transition<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.action.hash(state);
        self.next_state.hash(state);
    }
}

/// Represents a state in an automaton.
///
/// A state can have multiple transitions and may be marked as a final state.
pub struct State<'a> {
    /// The transitions originating from this state.
    pub transitions: RefCell<Vec<&'a Transition<'a>>>,
    /// Indicates whether this state is a final (accepting) state.
    pub is_final: bool,
}

impl<'a> PartialEq for State<'a> {
    fn eq(&self, other: &Self) -> bool {
        // We utilize the comparison based on the memory address of the state
        self as *const _ == other as *const _
    }
}

impl<'a> Eq for State<'a> {}

impl<'a> Debug for State<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "State({:?}, is_final: {})",
            self as *const _, self.is_final
        )
    }
}

impl<'a> Hash for State<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Implement a simple hash function for the State
        state.write_usize(self as *const _ as usize);
    }
}

/// Represents a finite automaton.
///
/// An automaton consists of a set of states, transitions, and initial states.
pub struct Automata<'a> {
    /// The arena for allocating states.
    pub states: &'a Arena<State<'a>>,
    /// The arena for allocating transitions.
    pub transitions: &'a Arena<Transition<'a>>,
    /// The initial states of the automaton.
    pub initial_states: Vec<&'a State<'a>>,
}

impl<'a> Automata<'a> {
    /// Creates a new automaton.
    ///
    /// # Arguments
    ///
    /// * `states` - An arena for managing state allocations.
    /// * `transitions` - An arena for managing transition allocations.
    ///
    /// # Returns
    ///
    /// A new `Automata` instance.
    pub fn new(states: &'a Arena<State<'a>>, transitions: &'a Arena<Transition<'a>>) -> Self {
        Self {
            states,
            transitions,
            initial_states: Vec::new(),
        }
    }

    /// Adds a new state to the automaton.
    ///
    /// # Arguments
    ///
    /// * `is_initial` - Whether the state is an initial state.
    /// * `is_final` - Whether the state is a final (accepting) state.
    ///
    /// # Returns
    ///
    /// A reference to the newly created state.
    pub fn add_state(&mut self, is_initial: bool, is_final: bool) -> &'a State<'a> {
        let state = self.states.alloc(State {
            transitions: RefCell::new(Vec::new()),
            is_final,
        });
        if is_initial {
            self.initial_states.push(state);
        }
        state
    }

    /// Adds a transition between two states.
    ///
    /// # Arguments
    ///
    /// * `from` - The state from which the transition originates.
    /// * `action` - The action that triggers this transition. We use the empty string for unobservable transitions.
    /// * `to` - The state to which the transition leads.
    ///
    /// # Returns
    ///
    /// A reference to the newly created transition.
    pub fn add_transition(
        &self,
        from: &'a State<'a>,
        action: Vec<String>,
        to: &'a State<'a>,
    ) -> &'a Transition<'a> {
        let transition = self.transitions.alloc(Transition {
            action,
            next_state: to,
        });
        from.transitions.borrow_mut().push(transition);
        transition
    }

    /// Returns the length of the shortest accepted word in the automaton using BFS.
    pub fn shortest_accepted_word_length(&self, track: usize) -> Option<usize> {
        use std::collections::HashSet;
        use std::collections::VecDeque;

        // Track visited states to avoid cycles
        let mut visited = HashSet::new();
        // Queue of (state, word) pairs for BFS
        let mut queue = VecDeque::new();

        // Start from all initial states
        for &initial_state in &self.initial_states {
            queue.push_back((initial_state, 0));
            visited.insert((initial_state as *const _, 0));
        }

        let mut shortest_length = None;

        while let Some((current_state, length)) = queue.pop_front() {
            // If we've found a final state, we've found the shortest word
            if current_state.is_final {
                // Update the shortest length
                if shortest_length.is_none() {
                    shortest_length = Some(length);
                } else {
                    shortest_length = Some(std::cmp::min(shortest_length.unwrap(), length));
                }
            }
            // Skip if the current word is longer than the shortest length we've found so far
            if let Some(best) = shortest_length {
                if length >= best {
                    continue;
                }
            }

            // Explore all transitions from current state
            for &transition in current_state.transitions.borrow().iter() {
                let next_state = transition.next_state;
                let next_state_ptr = next_state as *const _;

                if !visited.contains(&(next_state_ptr, length)) {
                    let mut new_length = length;
                    if transition.action[track] != "" {
                        new_length += 1;
                    }
                    visited.insert((next_state_ptr, new_length));
                    queue.push_back((next_state, new_length));
                }
            }
        }

        shortest_length
    }

    /// Returns the length-n prefixes of the accepted words
    /// Assumes that all the states are reachable to the final state
    pub fn accepted_prefixes(&self, track: usize, n: usize) -> HashSet<Vec<String>> {
        let mut prefixes = HashSet::new();

        // Track visited states to avoid cycles
        let mut visited = HashSet::new();
        // Queue of (state, word) pairs for BFS
        let mut queue = VecDeque::new();

        for &initial_state in &self.initial_states {
            queue.push_back((initial_state, vec![]));
            visited.insert((initial_state as *const _, 0));
        }

        while let Some((current_state, prefix)) = queue.pop_front() {
            if prefix.len() == n {
                prefixes.insert(prefix);
                continue;
            }

            for &transition in current_state.transitions.borrow().iter() {
                let next_state = transition.next_state;
                let mut new_prefix = prefix.clone();
                if transition.action[track] != "" {
                    new_prefix.push(transition.action[track].clone());
                }
                if !visited.contains(&(next_state as *const _, new_prefix.len())) {
                    visited.insert((next_state as *const _, new_prefix.len()));
                    queue.push_back((next_state, new_prefix));
                }
            }
        }

        prefixes
    }

    /// Removes transitions to states not reachable to any final states
    pub fn remove_unreachable_transitions(&self) {
        // Compute the states reachable from initial states
        let mut reachable_states = HashSet::new();
        for state in self.initial_states.clone() {
            reachable_states.insert(state);
        }
        let mut last_reachable_states_size = reachable_states.len();
        loop {
            let mut new_states = HashSet::new();
            for state in reachable_states.clone() {
                for transition in state.transitions.borrow().iter() {
                    let next_state = transition.next_state;
                    new_states.insert(next_state);
                }
            }
            reachable_states.extend(new_states);
            if reachable_states.len() == last_reachable_states_size {
                break;
            }
            last_reachable_states_size = reachable_states.len();
        }

        // Compute the states reachable to final states
        let mut useful_states = HashSet::new();
        for state in reachable_states.clone() {
            if state.is_final {
                useful_states.insert(state);
            }
        }
        let mut last_useful_states_size = useful_states.len();
        loop {
            let mut new_states = HashSet::new();
            for state in reachable_states.clone() {
                for transition in state.transitions.borrow().iter() {
                    if useful_states.contains(&transition.next_state) {
                        new_states.insert(state);
                    }
                }
            }
            useful_states.extend(new_states);
            if useful_states.len() == last_useful_states_size {
                break;
            }
            last_useful_states_size = useful_states.len();
        }

        // Remove the transitions to non-useful states
        for state in reachable_states.clone() {
            let mut transitions = state.transitions.borrow_mut();
            transitions.retain(|transition| useful_states.contains(&transition.next_state));
        }
    }
}

impl<'a> PartialEq for Automata<'a> {
    fn eq(&self, other: &Self) -> bool {
        // We utilize the comparison based on the memory address of the Automata
        self as *const _ == other as *const _
    }
}

impl<'a> Eq for Automata<'a> {}

impl<'a> Hash for Automata<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Implement a simple hash function for the Automata
        self.initial_states.hash(state);
    }
}

impl<'a> Debug for Automata<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Automata({:?})", self as *const _)
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Import everything from the parent module
    use typed_arena::Arena;

    /// Tests that a state can be added to the automaton.
    #[test]
    fn test_add_state() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = Automata::new(&state_arena, &transition_arena);

        let state = automata.add_state(true, false);
        assert!(!state.is_final); // Verify the state is not final
        assert!(state.transitions.borrow().is_empty()); // Verify the state has no transitions
    }

    /// Tests that a transition can be added to the automaton.
    #[test]
    fn test_add_transition() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = Automata::new(&state_arena, &transition_arena);

        let state1 = automata.add_state(true, false); // Initial, non-final state
        let state2 = automata.add_state(false, true); // Non-initial, final state

        let transition = automata.add_transition(state1, vec!["a".to_string()], state2);

        // Verify transition properties
        assert_eq!(transition.action, vec!["a"]);
        assert_eq!(transition.next_state, state2);

        // Verify state1's transitions
        let transitions = state1.transitions.borrow();
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].action, vec!["a"]);
        assert_eq!(transitions[0].next_state, state2);
    }

    /// Tests that initial states are correctly added to the automaton.
    #[test]
    fn test_initial_states() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = Automata::new(&state_arena, &transition_arena);

        let state1 = automata.add_state(true, false); // Initial, non-final state
        automata.add_state(false, true); // Non-initial, final state

        // Verify initial states
        assert_eq!(automata.initial_states.len(), 1);
        assert_eq!(automata.initial_states[0], state1);
    }

    #[test]
    fn test_shortest_accepted_word_length() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = Automata::new(&state_arena, &transition_arena);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_transition(s1, vec!["a".to_string(), "b".to_string()], s2);
        automaton.add_transition(s1, vec!["a".to_string(), "".to_string()], s1);
        automaton.add_transition(s1, vec!["".to_string(), "b".to_string()], s1);
        automaton.add_transition(s1, vec!["".to_string(), "".to_string()], s1);
        automaton.add_transition(s1, vec!["".to_string(), "d".to_string()], s3);

        assert_eq!(automaton.shortest_accepted_word_length(0), Some(0));
        assert_eq!(automaton.shortest_accepted_word_length(1), Some(1));

        automaton.add_transition(s1, vec!["".to_string(), "".to_string()], s2);
        automaton.add_transition(s2, vec!["".to_string(), "".to_string()], s3);

        assert_eq!(automaton.shortest_accepted_word_length(0), Some(0));
        assert_eq!(automaton.shortest_accepted_word_length(1), Some(0));
    }

    #[test]
    fn test_accepted_prefixes() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = Automata::new(&state_arena, &transition_arena);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);

        automaton.add_transition(s1, vec!["a".to_string(), "b".to_string()], s2);
        automaton.add_transition(s1, vec!["".to_string(), "b".to_string()], s1);
        automaton.add_transition(s1, vec!["c".to_string(), "d".to_string()], s3);
        automaton.add_transition(s2, vec!["".to_string(), "".to_string()], s3);

        assert_eq!(
            automaton.accepted_prefixes(0, 0),
            vec![Vec::<String>::new()].into_iter().collect()
        );
        assert_eq!(
            automaton.accepted_prefixes(0, 1),
            vec![vec!["a".to_string()], vec!["c".to_string()]]
                .into_iter()
                .collect()
        );

        assert_eq!(
            automaton.accepted_prefixes(1, 0),
            vec![Vec::<String>::new()].into_iter().collect()
        );
        assert_eq!(
            automaton.accepted_prefixes(1, 1),
            vec![vec!["b".to_string()], vec!["d".to_string()]]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn test_remove_unreachable_transitions() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automaton = Automata::new(&state_arena, &transition_arena);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);
        let s4 = automaton.add_state(false, false);

        automaton.add_transition(s1, vec!["a".to_string(), "b".to_string()], s2);
        automaton.add_transition(s1, vec!["a".to_string(), "".to_string()], s4);
        automaton.add_transition(s2, vec!["".to_string(), "b".to_string()], s3);

        assert_eq!(s1.transitions.borrow().len(), 2);
        assert_eq!(s2.transitions.borrow().len(), 1);
        assert_eq!(s3.transitions.borrow().len(), 0);
        assert_eq!(s4.transitions.borrow().len(), 0);

        automaton.remove_unreachable_transitions();

        assert_eq!(s1.transitions.borrow().len(), 1);
        assert_eq!(s2.transitions.borrow().len(), 1);
        assert_eq!(s3.transitions.borrow().len(), 0);
        assert_eq!(s4.transitions.borrow().len(), 0);
    }
}
