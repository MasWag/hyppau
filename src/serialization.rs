use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Debug,
};
use typed_arena::Arena;

// Import your NFA types from automata.rs
use crate::automata::{Automata, State, Transition, TransitionCost, ValidLabel};

#[derive(Serialize, Deserialize)]
struct SerializedAutomata<L> {
    dimensions: usize,
    states: Vec<SerializedState>,
    transitions: Vec<SerializedTransition<L>>,
}

#[derive(Serialize, Deserialize)]
struct SerializedState {
    id: usize,
    is_initial: bool,
    is_final: bool,
}

#[derive(Serialize, Deserialize)]
struct SerializedTransition<L> {
    from: usize,
    to: usize,
    label: L,
}

/// Serializes the given NFA into a JSON string.
///
/// The serialization traverses the automata (starting from the initial states),
/// assigns an ID to each reachable state, and then outputs the states and transitions.
pub fn serialize_nfa<'a, L: Serialize + Clone>(automata: &Automata<'a, L>) -> String {
    // Mapping from state pointer to (state reference, assigned id)
    let mut state_ids: HashMap<*const State<'a, L>, (&State<'a, L>, usize)> = HashMap::new();
    let mut queue: VecDeque<&State<'a, L>> = VecDeque::new();

    // Start from each initial state.
    for state in &automata.initial_states {
        queue.push_back(state);
    }

    let mut next_id = 0;
    while let Some(state) = queue.pop_front() {
        let ptr = state as *const State<'a, L>;
        if state_ids.contains_key(&ptr) {
            continue;
        }
        state_ids.insert(ptr, (state, next_id));
        next_id += 1;
        // Queue all adjacent states.
        for t in state.transitions.borrow().iter() {
            queue.push_back(t.next_state);
        }
    }

    // Create a set of initial state pointers for easy lookup.
    let initial_ptrs: HashSet<*const State<'a, L>> = automata
        .initial_states
        .iter()
        .map(|s| *s as *const State<'a, L>)
        .collect();

    // Build the vector of serialized states.
    let mut states_vec = Vec::new();
    // Sorting states by their assigned id for a deterministic order.
    let mut states_by_id: Vec<(&State<'a, L>, usize)> = state_ids.values().cloned().collect();
    states_by_id.sort_by_key(|&(_, id)| id);
    for (state, id) in states_by_id {
        states_vec.push(SerializedState {
            id,
            is_initial: initial_ptrs.contains(&(state as *const State<'a, L>)),
            is_final: state.is_final,
        });
    }

    // Build the vector of serialized transitions.
    let mut transitions_vec = Vec::new();
    for (&_ptr, &(state, id)) in &state_ids {
        for t in state.transitions.borrow().iter() {
            let target_ptr = t.next_state as *const State<'a, L>;
            let target_id = state_ids
                .get(&target_ptr)
                .expect("Target state not found in state_ids")
                .1;
            transitions_vec.push(SerializedTransition::<L> {
                from: id,
                to: target_id,
                label: t.label.clone(),
            });
        }
    }

    let serialized = SerializedAutomata {
        dimensions: automata.dimensions,
        states: states_vec,
        transitions: transitions_vec,
    };

    serde_json::to_string_pretty(&serialized).expect("Serialization failed")
}

/// Deserializes a JSON string into an NFA.
///
/// # Arguments
/// * `input` - A JSON string representing the automata.
/// * `state_arena` - An arena for allocating `State` objects.
/// * `trans_arena` - An arena for allocating `Transition` objects.
///
/// # Panics
///
/// Panics if JSON parsing fails or if a transition refers to an invalid state.
pub fn deserialize_nfa<'a, L: Deserialize<'a> + TransitionCost + ValidLabel>(
    input: &'a str,
    state_arena: &'a Arena<State<'a, L>>,
    trans_arena: &'a Arena<Transition<'a, L>>,
) -> Automata<'a, L> {
    let ser: SerializedAutomata<L> =
        serde_json::from_str(input).expect("Failed to deserialize NFA from JSON");

    let mut automata = Automata::new(state_arena, trans_arena, ser.dimensions);
    let num_states = ser.states.len();
    let mut id_to_state: Vec<Option<&'a State<'a, L>>> = vec![None; num_states];

    // Create states in the automata.
    for s in ser.states {
        if s.id >= num_states {
            panic!("State id {} out of range", s.id);
        }
        let state = automata.add_state(s.is_initial, s.is_final);
        id_to_state[s.id] = Some(state);
    }

    // Add transitions using the id-to-state mapping.
    for t in ser.transitions {
        let from_state =
            id_to_state[t.from].expect(&format!("Invalid 'from' state id: {}", t.from));
        let to_state = id_to_state[t.to].expect(&format!("Invalid 'to' state id: {}", t.to));
        automata.add_transition(from_state, t.label, to_state);
    }

    automata
}

/// Generates a DOT representation of the given NFA suitable for Graphviz.
///
/// Each state is assigned a unique identifier (based on a BFS from the initial states).
/// Final states are drawn with a `doublecircle` shape, while non-final states use a `circle`.
/// An invisible __start__ node points to all initial states.
pub fn automaton_to_dot<'a, L: Debug>(automata: &Automata<'a, L>) -> String {
    // Map each state's pointer to a unique id and store the state pointers.
    let mut state_ids: HashMap<*const State<'a, L>, usize> = HashMap::new();
    let mut id_to_state: Vec<&State<'a, L>> = Vec::new();
    let mut queue: VecDeque<&State<'a, L>> = VecDeque::new();

    // Enqueue initial states.
    for &state in &automata.initial_states {
        let ptr = state as *const State<'a, L>;
        if !state_ids.contains_key(&ptr) {
            state_ids.insert(ptr, id_to_state.len());
            id_to_state.push(state);
            queue.push_back(state);
        }
    }

    // Traverse reachable states.
    while let Some(state) = queue.pop_front() {
        for t in state.transitions.borrow().iter() {
            let next_state = t.next_state;
            let ptr = next_state as *const State<'a, L>;
            if !state_ids.contains_key(&ptr) {
                state_ids.insert(ptr, id_to_state.len());
                id_to_state.push(next_state);
                queue.push_back(next_state);
            }
        }
    }

    let mut dot = String::new();
    dot.push_str("digraph NFA {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [shape=circle];\n");
    dot.push_str("  __start__ [shape=point];\n");

    // Create edges from the invisible __start__ to all initial states.
    for &state in &automata.initial_states {
        let id = state_ids[&(state as *const State<'a, L>)];
        dot.push_str(&format!("  __start__ -> state{};\n", id));
    }

    // Define nodes.
    for (id, state) in id_to_state.iter().enumerate() {
        // Use doublecircle for final states.
        let shape = if state.is_final {
            "doublecircle"
        } else {
            "circle"
        };
        dot.push_str(&format!(
            "  state{} [label=\"State {}\", shape={}];\n",
            id, id, shape
        ));
    }

    // Define edges for transitions.
    for (id, state) in id_to_state.iter().enumerate() {
        for t in state.transitions.borrow().iter() {
            let target_id = state_ids[&(t.next_state as *const State<'a, L>)];
            dot.push_str(&format!(
                "  state{} -> state{} [label=\"{:?}\"];\n",
                id, target_id, t.label
            ));
        }
    }

    dot.push_str("}\n");
    dot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::{Automata, NFAHState, NFAHTransition};
    use std::collections::{HashSet, VecDeque};
    use typed_arena::Arena;

    #[test]
    fn test_serialize_deserialize() {
        // Create arenas for the original automata.
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        // Build an automata with 2 dimensions.
        let mut automata = Automata::new(&state_arena, &trans_arena, 2);

        // Create states: s0 (initial), s1 (final), and s2 (non-final).
        let s0 = automata.add_state(true, false);
        let s1 = automata.add_state(false, true);
        let s2 = automata.add_state(false, false);

        // Add transitions:
        // s0 --("a", 0)--> s1
        // s0 --("b", 1)--> s2
        // s1 --("c", 0)--> s2
        // s2 --("d", 1)--> s0
        automata.add_nfah_transition(s0, "a".to_string(), 0, s1);
        automata.add_nfah_transition(s0, "b".to_string(), 1, s2);
        automata.add_nfah_transition(s1, "c".to_string(), 0, s2);
        automata.add_nfah_transition(s2, "d".to_string(), 1, s0);

        // Serialize the automata to JSON.
        let serialized = serialize_nfa(&automata);
        // Uncomment for debugging:
        // println!("Serialized NFA:\n{}", serialized);

        // Create new arenas for the deserialized automata.
        let new_state_arena = Arena::new();
        let new_trans_arena = Arena::new();
        let deserialized = deserialize_nfa(&serialized, &new_state_arena, &new_trans_arena);

        // Check that the dimensions match.
        assert_eq!(automata.dimensions, deserialized.dimensions);

        // Count reachable states in the deserialized automata using BFS.
        let mut seen_states: HashSet<*const NFAHState> = HashSet::new();
        let mut queue = VecDeque::new();
        for &init in deserialized.initial_states.iter() {
            queue.push_back(init);
        }
        while let Some(state) = queue.pop_front() {
            let ptr = state as *const NFAHState;
            if seen_states.contains(&ptr) {
                continue;
            }
            seen_states.insert(ptr);
            for t in state.transitions.borrow().iter() {
                queue.push_back(t.next_state);
            }
        }
        // We expect 3 states.
        assert_eq!(seen_states.len(), 3);

        // Count transitions in the deserialized automata.
        let mut seen_transitions: HashSet<*const NFAHTransition> = HashSet::new();
        let mut queue = VecDeque::new();
        for &init in deserialized.initial_states.iter() {
            queue.push_back(init);
        }
        while let Some(state) = queue.pop_front() {
            for &t in state.transitions.borrow().iter() {
                let t_ptr = t as *const NFAHTransition;
                if !seen_transitions.insert(t_ptr) {
                    continue;
                }
                queue.push_back(t.next_state);
            }
        }
        // We expect 4 transitions.
        assert_eq!(seen_transitions.len(), 4);
    }

    #[test]
    fn test_automaton_to_dot() {
        let state_arena = Arena::new();
        let transition_arena = Arena::new();
        let mut automata = Automata::new(&state_arena, &transition_arena, 1);

        // Create states: s0 (initial), s1 (final), s2 (non-final).
        let s0 = automata.add_state(true, false);
        let s1 = automata.add_state(false, true);
        let s2 = automata.add_state(false, false);

        // Add transitions:
        // s0 --("a", 0)--> s1
        // s1 --("b", 0)--> s2
        // s2 --("c", 0)--> s0
        automata.add_nfah_transition(s0, "a".to_string(), 0, s1);
        automata.add_nfah_transition(s1, "b".to_string(), 0, s2);
        automata.add_nfah_transition(s2, "c".to_string(), 0, s0);

        let dot = automaton_to_dot(&automata);
        // Uncomment for debugging:
        // println!("DOT representation:\n{}", dot);

        // Basic checks on the DOT output.
        assert!(dot.contains("digraph NFA {"));
        assert!(dot.contains("__start__ -> state0;"));
        assert!(dot.contains("state0 [label=\"State 0\", shape=circle];"));
        assert!(dot.contains("state1 [label=\"State 1\", shape=doublecircle];"));
        assert!(dot.contains("state2 [label=\"State 2\", shape=circle];"));
        assert!(dot.contains("state0 -> state1 [label=\"(\"a\", 0)\"];"));
        assert!(dot.contains("state1 -> state2 [label=\"(\"b\", 0)\"];"));
        assert!(dot.contains("state2 -> state0 [label=\"(\"c\", 0)\"];"));
    }
}
