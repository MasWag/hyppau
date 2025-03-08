use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use typed_arena::Arena;

/// Represents a transition for an NFA.
#[derive(Debug, PartialEq, Clone)]
pub struct Transition<'a, L> {
    pub label: L,
    pub next_state: &'a State<'a, L>,
}

impl<L: Hash> Hash for Transition<'_, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.label.hash(state);
        self.next_state.hash(state);
    }
}

/// Represents a transition for an NFA over Σ x Vars.
///
/// Each transition is labeled by a pair (letter, var) where 'letter'
/// is from the alphabet Σ and 'var' identifies the variable.
pub type NFAHTransition<'a> = Transition<'a, (String, usize)>;

/// Represents a state in an NFA.
///
/// Stores whether it is final (accepting) and its outgoing transitions.
pub struct State<'a, L> {
    /// Outgoing transitions.
    pub transitions: RefCell<Vec<&'a Transition<'a, L>>>,
    /// Whether this state is an accepting state.
    pub is_final: bool,
}

impl<'a, L> State<'a, L> {
    /// Returns a reference to the transitions.
    pub fn get_transitions(&self) -> std::cell::Ref<Vec<&'a Transition<'a, L>>> {
        self.transitions.borrow()
    }

    /// Adds a transition to this state.
    pub fn add_transition(&self, transition: &'a Transition<'a, L>) {
        self.transitions.borrow_mut().push(transition);
    }
}

impl<L> PartialEq for State<'_, L> {
    fn eq(&self, other: &Self) -> bool {
        // States compared by their pointer address.
        std::ptr::eq(self, other)
    }
}

impl<L> Eq for State<'_, L> {}

impl<L> Debug for State<'_, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State({:p}, is_final: {})", self, self.is_final)
    }
}

impl<L> Hash for State<'_, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Simple hash based on the pointer.
        state.write_usize(self as *const _ as usize);
    }
}

pub type NFAHState<'a> = State<'a, (String, usize)>;
pub type NFAState<'a> = State<'a, String>;

/// Represents an NFA over Σ x Vars.
pub type NFAH<'a> = Automata<'a, (String, usize)>;
/// Represents an NFA over Σ.
pub type NFA<'a> = Automata<'a, String>;
/// An epsilon-NFA (where transitions are labeled by `Some(symbol)` or `None` for ε).
pub type EpsilonNFA<'a> = Automata<'a, Option<String>>;

pub trait ValidLabel {
    /// Checks that the label is valid given an optional dimension.
    /// For automata over Σ×Vars, the dimension is required.
    /// For automata over Σ, the dimension is ignored.
    fn validate(&self, dimensions: usize);
}

impl ValidLabel for (String, usize) {
    fn validate(&self, dimensions: usize) {
        let (_, var) = self;
        if *var >= dimensions {
            panic!("Variable index out of bounds");
        }
    }
}

impl ValidLabel for String {
    fn validate(&self, _dimensions: usize) {
        // No validity check is necessary for simple letter labels.
    }
}

impl ValidLabel for Option<String> {
    fn validate(&self, _dimensions: usize) {
        // No validity check is necessary for simple letter labels.
    }
}

impl ValidLabel for char {
    fn validate(&self, _dimensions: usize) {
        // No validity check is necessary for simple letter labels.
    }
}

pub struct Automata<'a, L> {
    /// Arena for `State` allocations.
    pub states: &'a Arena<State<'a, L>>,
    /// Arena for `Transition` allocations.
    pub transitions: &'a Arena<Transition<'a, L>>,
    /// The initial states.
    pub initial_states: Vec<&'a State<'a, L>>,
    /// The number of variables.
    pub dimensions: usize,
}

impl<'a, L: Eq + Hash + Clone + ValidLabel> Automata<'a, L> {
    /// Creates a new automaton.
    pub fn new(
        states: &'a Arena<State<'a, L>>,
        transitions: &'a Arena<Transition<'a, L>>,
        dimension: usize,
    ) -> Self {
        Self {
            states,
            transitions,
            initial_states: Vec::new(),
            dimensions: dimension,
        }
    }

    /// Adds a new state to the automaton.
    ///
    /// # Arguments
    /// * `is_initial` - whether this state is one of the initial states
    /// * `is_final` - whether this state is accepting
    pub fn add_state(&mut self, is_initial: bool, is_final: bool) -> &'a State<'a, L> {
        let state = self.states.alloc(State {
            transitions: RefCell::new(Vec::new()),
            is_final,
        });
        if is_initial {
            self.initial_states.push(state);
        }
        state
    }

    /// Adds a transition (action, var) from `from` to `to`.
    pub fn add_transition(
        &self,
        from: &'a State<'a, L>,
        label: L,
        to: &'a State<'a, L>,
    ) -> &'a Transition<'a, L> {
        label.validate(self.dimensions);
        let transition = self.transitions.alloc(Transition {
            label,
            next_state: to,
        });
        from.add_transition(transition);
        transition
    }

    /// Returns the length of the shortest accepted word in the automaton using BFS.
    pub fn shortest_accepted_word_length(&self) -> usize {
        // (state, current_length) is the BFS node;
        let mut queue = VecDeque::with_capacity(self.initial_states.len());
        let mut visited = HashSet::with_capacity(self.states.len());

        // Start from all initial states
        for &initial_state in &self.initial_states {
            queue.push_back((initial_state, 0));
            visited.insert((initial_state as *const _, 0));
        }

        let mut shortest_length: Option<usize> = None;

        while let Some((current_state, length)) = queue.pop_front() {
            // If current_state is final, update the shortest_length length
            if current_state.is_final {
                // Update the shortest length
                match shortest_length {
                    None => shortest_length = Some(length),
                    Some(prev) if length < prev => shortest_length = Some(length),
                    _ => {}
                }
            }

            // Skip if the current word is longer than the shortest length we've found so far
            if let Some(best) = shortest_length {
                if length >= best {
                    continue;
                }
            }

            // Explore all transitions from current state
            for &transition in current_state.get_transitions().iter() {
                let next_state = transition.next_state;
                let next_state_ptr = next_state as *const _;

                if !visited.contains(&(next_state_ptr, length)) {
                    visited.insert((next_state_ptr, length + 1));
                    queue.push_back((next_state, length + 1));
                }
            }
        }

        shortest_length.unwrap()
    }

    /// Returns all prefixes of length `n` that can appear along a path from any initial state.
    ///
    /// We store the entire label `(action, var)` in the prefix.
    pub fn accepted_prefixes(&self, n: usize) -> HashSet<Vec<L>> {
        // Estimate capacity for collections
        let estimated_capacity = self.states.len() * n;
        let mut prefixes = HashSet::with_capacity(estimated_capacity);
        let mut queue = VecDeque::with_capacity(self.initial_states.len());
        let mut visited = HashSet::with_capacity(estimated_capacity);

        // Initialize BFS from each initial state with empty prefix
        for &init in &self.initial_states {
            queue.push_back((init, Vec::<L>::with_capacity(n), 0));
            // we add length=0 for BFS
            visited.insert((init as *const _, vec![]));
        }

        while let Some((current_state, prefix, length)) = queue.pop_front() {
            // If we have a prefix of length `n`, store it
            if length == n {
                prefixes.insert(prefix);
                // do not explore any deeper from here
                continue;
            }

            // Explore outgoing transitions
            for &transition in current_state.get_transitions().iter() {
                let new_length = length + 1;
                if new_length <= n {
                    // Build the new prefix with reduced cloning
                    let mut new_prefix = Vec::with_capacity(prefix.len() + 1);
                    new_prefix.extend_from_slice(&prefix);
                    new_prefix.push(transition.label.clone());

                    let next_ptr = transition.next_state as *const _;
                    if !visited.contains(&(next_ptr, new_prefix.clone())) {
                        visited.insert((next_ptr, new_prefix.clone()));
                        queue.push_back((transition.next_state, new_prefix, new_length));
                    }
                }
            }
        }

        prefixes
    }

    /// Removes transitions to states that cannot lead to a final state.
    /// A standard "reverse" reachability: keep only states from which
    /// a final state is reachable, removing transitions that lead to
    /// states outside that set.
    pub fn remove_unreachable_transitions(&self) {
        // 1) Collect all states reachable from an initial state
        let mut reachable = HashSet::with_capacity(self.states.len());
        let mut worklist = VecDeque::with_capacity(self.initial_states.len());

        for &init in &self.initial_states {
            reachable.insert(init);
            worklist.push_back(init);
        }

        while let Some(current_state) = worklist.pop_front() {
            for &transition in current_state.get_transitions().iter() {
                let next = transition.next_state;
                if !reachable.contains(&next) {
                    reachable.insert(next);
                    worklist.push_back(next);
                }
            }
        }

        // 2) Among these, keep only states from which some final state is reachable.
        //    We do a backward search from final states among the "reachable" set.
        let mut can_reach_final = HashSet::with_capacity(reachable.len());
        let mut final_queue = VecDeque::with_capacity(reachable.len() / 2);

        // Start from final states (that are in `reachable`)
        for &current_state in &reachable {
            if current_state.is_final {
                can_reach_final.insert(current_state);
                final_queue.push_back(current_state);
            }
        }

        // Go backwards: if `X` transitions to `Y` and `Y` is known to reach final,
        // then `X` also can reach final.
        loop {
            let before = can_reach_final.len();
            let mut newly_added = Vec::with_capacity(reachable.len() - can_reach_final.len());
            for &current_state in &reachable {
                if can_reach_final.contains(current_state) {
                    // skip
                    continue;
                }
                // If current_state transitions to some state in can_reach_final, add current_state
                let trans_out = current_state.get_transitions();
                if trans_out
                    .iter()
                    .any(|t| can_reach_final.contains(&t.next_state))
                {
                    newly_added.push(current_state);
                }
            }
            for current_state in newly_added {
                can_reach_final.insert(current_state);
                final_queue.push_back(current_state);
            }
            if can_reach_final.len() == before {
                break;
            }
        }

        // 3) Remove transitions that lead to states not in can_reach_final.
        for &current_state in &reachable {
            let mut trans_out = current_state.transitions.borrow_mut();
            trans_out.retain(|t| can_reach_final.contains(&t.next_state));
        }
    }
}

impl<L> Automata<'_, L> {
    /// Returns `true` if this automaton's language is empty
    /// (i.e., if no final state can be reached from any initial state).
    /// Otherwise, returns `false`.
    pub fn is_empty(&self) -> bool {
        // If there are no initial states, it's trivially empty.
        if self.initial_states.is_empty() {
            return true;
        }

        let mut visited = HashSet::with_capacity(self.states.len());
        let mut queue = VecDeque::with_capacity(self.initial_states.len());

        // Initialize queue with all initial states
        for &init in &self.initial_states {
            // If an initial state is already final, language is non-empty
            if init.is_final {
                return false;
            }
            queue.push_back(init);
            visited.insert(init as *const _);
        }

        // BFS over the transition graph
        while let Some(state) = queue.pop_front() {
            for &transition in state.get_transitions().iter() {
                let next = transition.next_state;
                if next.is_final {
                    // Found a final state reachable => language is non-empty
                    return false;
                }
                let next_ptr = next as *const _;
                if !visited.contains(&next_ptr) {
                    visited.insert(next_ptr);
                    queue.push_back(next);
                }
            }
        }

        // No final state encountered => empty
        true
    }
}

impl<'a, L> Automata<'a, L>
where
    L: Clone,
{
    /// Returns a set of states in `dfa` that are reachable in exactly `steps` transitions
    /// from any of dfa's initial states.
    pub fn states_reachable_in_exactly_n_steps(&self, steps: usize) -> HashSet<&'a State<'a, L>> {
        // We do BFS layer by layer
        let mut current_layer: HashSet<&State<'a, L>> =
            HashSet::with_capacity(self.initial_states.len());
        current_layer.extend(self.initial_states.iter().copied());

        let mut next_layer = HashSet::with_capacity(self.states.len());

        for _ in 0..steps {
            next_layer.clear();
            // move from each state in current_layer by any outgoing transition
            for st in &current_layer {
                for &trans in st.get_transitions().iter() {
                    next_layer.insert(trans.next_state);
                }
            }
            std::mem::swap(&mut current_layer, &mut next_layer);
        }

        current_layer
    }
}

impl<'a, L> Automata<'a, L> {
    pub fn iter_states(&'a self) -> AutomataStateIter<'a, L> {
        AutomataStateIter::new(self)
    }
}

pub struct AutomataStateIter<'a, L> {
    seen: HashSet<*const State<'a, L>>,
    queue: VecDeque<&'a State<'a, L>>,
}

impl<'a, L> AutomataStateIter<'a, L> {
    pub fn new(automata: &'a Automata<'a, L>) -> Self {
        let initial_count = automata.initial_states.len();
        let mut seen = HashSet::with_capacity(automata.states.len());
        let mut queue = VecDeque::with_capacity(initial_count);

        // Pre-populate the queue with initial states
        for &state in &automata.initial_states {
            queue.push_back(state);
            seen.insert(state as *const _);
        }

        Self { seen, queue }
    }
}

impl<'a, L: Clone> Iterator for AutomataStateIter<'a, L> {
    type Item = &'a State<'a, L>;
    fn next(&mut self) -> Option<Self::Item> {
        let next_state = self.queue.pop_front();
        if let Some(next) = next_state {
            // We already added this state to seen in new() or in a previous iteration
            next.get_transitions().iter().for_each(|transition| {
                let next_ptr = transition.next_state as *const _;
                if !self.seen.contains(&next_ptr) {
                    self.seen.insert(next_ptr);
                    self.queue.push_back(transition.next_state);
                }
            });
        }

        next_state
    }
}

impl<L> PartialEq for Automata<'_, L> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl<L> Eq for Automata<'_, L> {}

impl<L> Hash for Automata<'_, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Just hash based on the pointer to self’s initial_states for simplicity
        self.initial_states.hash(state);
    }
}
impl<L> Debug for Automata<'_, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NFAH({:p})", self)
    }
}

impl<'a, L: Eq + Hash + Clone + ValidLabel> Automata<'a, L> {
    /// Builds the (intersection) product automaton of `self` and `other`.
    ///
    /// Both NFAs must be over the same alphabet type (`String`) with the same dimensions.
    /// The resulting automaton is also an `NFA<'b>` using new arenas.
    ///
    /// Intersection semantics:
    ///  - The product state is final if both component states are final.
    ///  - A transition with label ℓ from (s1, s2) to (t1, t2) exists if
    ///    there is a transition labeled ℓ from s1 -> t1 in `self`
    ///    **and** a transition labeled ℓ from s2 -> t2 in `other`.
    pub fn product<'b>(
        &self,
        other: &Automata<'a, L>,
        new_states_arena: &'b Arena<State<'b, L>>,
        new_trans_arena: &'b Arena<Transition<'b, L>>,
    ) -> Automata<'b, L> {
        if self.dimensions != other.dimensions {
            panic!(
                "The two automata must have the same dimensions: expected {}, got {}",
                self.dimensions, other.dimensions
            );
        }
        // create the new automaton (product automaton).
        let mut product_automata =
            Automata::<L>::new(new_states_arena, new_trans_arena, self.dimensions);

        // We'll map (s1_ptr, s2_ptr) -> newly created product state.
        let mut pair_to_state = HashMap::new();
        let mut queue = VecDeque::new();

        // 1) Create product initial states from all pairs of (init1, init2)
        for &init1 in &self.initial_states {
            for &init2 in &other.initial_states {
                let is_final = init1.is_final && init2.is_final;
                let prod_init = product_automata.add_state(true, is_final);
                pair_to_state.insert((init1 as *const _, init2 as *const _), prod_init);
                queue.push_back((init1, init2));
            }
        }

        // 2) BFS in the space of (s1, s2) pairs
        while let Some((old_s1, old_s2)) = queue.pop_front() {
            // The product state we already created:
            let new_current = pair_to_state[&(old_s1 as *const _, old_s2 as *const _)];

            // Gather transitions by label for each side.
            let mut transitions_1: HashMap<&L, Vec<&State<'_, L>>> = HashMap::new();
            for &t1 in old_s1.transitions.borrow().iter() {
                transitions_1
                    .entry(&t1.label)
                    .or_default()
                    .push(t1.next_state);
            }

            let mut transitions_2: HashMap<&L, Vec<&State<'_, L>>> = HashMap::new();
            for &t2 in old_s2.transitions.borrow().iter() {
                transitions_2
                    .entry(&t2.label)
                    .or_default()
                    .push(t2.next_state);
            }

            // For each label that appears in both transition sets, cross-product all possible next states.
            for (lbl, nexts1) in &transitions_1 {
                if let Some(nexts2) = transitions_2.get(lbl) {
                    for &n1 in nexts1 {
                        for &n2 in nexts2 {
                            let key = (n1 as *const _, n2 as *const _);
                            let new_next = match pair_to_state.get(&key) {
                                Some(&existing) => existing,
                                None => {
                                    let is_fin = n1.is_final && n2.is_final;
                                    let st_new = product_automata.add_state(false, is_fin);
                                    pair_to_state.insert(key, st_new);
                                    queue.push_back((n1, n2));
                                    st_new
                                }
                            };
                            // Add the transition in the product.
                            product_automata.add_transition(new_current, (*lbl).clone(), new_next);
                        }
                    }
                }
            }
        }

        product_automata
    }

    /// Concatenates `self` with `other`, building the new automata in the provided arenas.
    ///
    /// The resulting automata accepts a word w if it can be split as uv with u accepted by `self`
    /// and v accepted by `other`. We avoid using ε‐transitions by “jumping” non-deterministically
    /// from every final state in `self` to the successors of every initial state in `other`.
    pub fn concat<'b>(
        &'a self,
        other: &'a Automata<'a, L>,
        new_states_arena: &'b Arena<State<'b, L>>,
        new_trans_arena: &'b Arena<Transition<'b, L>>,
    ) -> Automata<'b, L> {
        if self.dimensions != other.dimensions {
            panic!(
                "Dimension mismatch in concatenation: {} vs {}",
                self.dimensions, other.dimensions
            );
        }
        // Create the new automaton.
        let mut new_aut = Automata::new(new_states_arena, new_trans_arena, self.dimensions);

        // Determine if the second automata accepts the empty word.
        // (A run in self may “jump” into other without consuming input only if
        // an initial state of `other` is final.)
        let other_accepts_empty = other.initial_states.iter().any(|s| s.is_final);
        // Also, if self accepts ε then concatenation should include words from other alone.
        let self_accepts_empty = self.initial_states.iter().any(|s| s.is_final);

        // We will copy the states from self and other into the new automata.
        // The mapping is from the pointer of an original state to its copy.
        let mut map_self: HashMap<*const State<'a, L>, &State<'b, L>> = HashMap::new();
        let mut map_other: HashMap<*const State<'a, L>, &State<'b, L>> = HashMap::new();

        // Copy all states from self.
        // A state from self will be final in the new automata only if it was final and
        // the second automata accepts ε (i.e. the run can finish in self).
        for state in self.iter_states() {
            let new_is_final = state.is_final && other_accepts_empty;
            let new_state = new_aut.add_state(false, new_is_final);
            map_self.insert(state as *const _, new_state);
        }

        // Copy all states from other.
        for state in other.iter_states() {
            let new_state = new_aut.add_state(false, state.is_final);
            map_other.insert(state as *const _, new_state);
        }

        // Set the new automata’s initial states.
        // Always include the initial states from self.
        for &s in &self.initial_states {
            new_aut.initial_states.push(map_self[&(s as *const _)]);
        }
        // If self accepts ε, then also include the initial states from other.
        if self_accepts_empty {
            for &s in &other.initial_states {
                new_aut.initial_states.push(map_other[&(s as *const _)]);
            }
        }

        // Copy transitions within self.
        for state in self.iter_states() {
            let new_from = map_self[&(state as *const _)];
            for &trans in state.get_transitions().iter() {
                let new_to = map_self[&(trans.next_state as *const _)];
                new_aut.add_transition(new_from, trans.label.clone(), new_to);
            }
        }

        // Copy transitions within other.
        for state in other.iter_states() {
            let new_from = map_other[&(state as *const _)];
            for &trans in state.get_transitions().iter() {
                let new_to = map_other[&(trans.next_state as *const _)];
                new_aut.add_transition(new_from, trans.label.clone(), new_to);
            }
        }

        // Add jump transitions.
        // For every state in self that is final (in the original automata),
        // add, for each initial state in other, every outgoing transition from that initial state.
        // These transitions “simulate” the ε-move in the usual construction,
        // by consuming the same letter that would be read from other.
        for state in self.iter_states() {
            if state.is_final {
                let new_from = map_self[&(state as *const _)];
                for &init_other in &other.initial_states {
                    for &trans in init_other.get_transitions().iter() {
                        let new_to = map_other[&(trans.next_state as *const _)];
                        new_aut.add_transition(new_from, trans.label.clone(), new_to);
                    }
                }
            }
        }

        new_aut
    }

    /// Returns a new automaton recognizing the Kleene star (A*) of the language of `self`.
    ///
    /// The new automaton is built in the given arenas. Its construction works by:
    /// 1. Copying all states and transitions from `self` into the new arenas.
    /// 2. For every state in `self` that is final, adding jump transitions to the transitions
    ///    emerging from each initial state of `self` (thus allowing repetition).
    /// 3. Adding a fresh new state (which is both initial and final) that “jumps into” the copy
    ///    of `self` via the outgoing transitions of each initial state.
    pub fn star<'b>(
        &'a self,
        new_states_arena: &'b Arena<State<'b, L>>,
        new_trans_arena: &'b Arena<Transition<'b, L>>,
    ) -> Automata<'b, L> {
        // Create the new automaton.
        let mut new_aut = Automata::new(new_states_arena, new_trans_arena, self.dimensions);

        // Create a mapping from each state in self to its copy in new_aut.
        let mut map_self: HashMap<*const State<'a, L>, &State<'b, L>> = HashMap::new();
        for state in self.iter_states() {
            // In the copied automaton, the finality is kept as in self.
            let new_state = new_aut.add_state(false, state.is_final);
            map_self.insert(state as *const _, new_state);
        }

        // Copy transitions from self.
        for state in self.iter_states() {
            let new_from = map_self[&(state as *const _)];
            for &trans in state.get_transitions().iter() {
                let new_to = map_self[&(trans.next_state as *const _)];
                new_aut.add_transition(new_from, trans.label.clone(), new_to);
            }
        }

        // Add jump transitions.
        // For every state in self that is final, add (non-deterministic) transitions that
        // simulate a jump into self by “injecting” each outgoing transition from every initial state.
        for state in self.iter_states() {
            if state.is_final {
                let new_from = map_self[&(state as *const _)];
                for &init in &self.initial_states {
                    for &trans in init.get_transitions().iter() {
                        let new_to = map_self[&(trans.next_state as *const _)];
                        new_aut.add_transition(new_from, trans.label.clone(), new_to);
                    }
                }
            }
        }

        // Add a new fresh state which will be the sole initial state.
        // This state is marked final (so that the empty word is accepted).
        let new_init = new_aut.add_state(true, true);
        // From the new initial state, add jump transitions based on the outgoing transitions
        // of each initial state of the original automaton.
        for &init in &self.initial_states {
            for &trans in init.get_transitions().iter() {
                let new_to = map_self[&(trans.next_state as *const _)];
                new_aut.add_transition(new_init, trans.label.clone(), new_to);
            }
        }
        // In star, the only initial state is the new one.
        new_aut.initial_states = vec![new_init];

        new_aut
    }

    /// Returns a new automaton recognizing the Kleene plus (A⁺) of the language of `self`.
    ///
    /// The new automaton is built in the given arenas. Its construction is similar to `star`
    /// except that we do not add a fresh initial state; rather, we keep the copy of self’s initial
    /// states as initial. In addition, jump transitions are added from every final state to the
    /// outgoing transitions of each initial state, allowing repeated occurrences.
    pub fn plus<'b>(
        &'a self,
        new_states_arena: &'b Arena<State<'b, L>>,
        new_trans_arena: &'b Arena<Transition<'b, L>>,
    ) -> Automata<'b, L> {
        // Create the new automaton.
        let mut new_aut = Automata::new(new_states_arena, new_trans_arena, self.dimensions);

        // Create a mapping from each state in self to its copy in new_aut.
        let mut map_self: HashMap<*const State<'a, L>, &State<'b, L>> = HashMap::new();
        for state in self.iter_states() {
            let new_state = new_aut.add_state(false, state.is_final);
            map_self.insert(state as *const _, new_state);
        }

        // Copy transitions from self.
        for state in self.iter_states() {
            let new_from = map_self[&(state as *const _)];
            for &trans in state.get_transitions().iter() {
                let new_to = map_self[&(trans.next_state as *const _)];
                new_aut.add_transition(new_from, trans.label.clone(), new_to);
            }
        }

        // Add jump transitions.
        // For every state in self that is final, add transitions to simulate restarting self:
        // for each initial state in self, for each outgoing transition from that initial state.
        for state in self.iter_states() {
            if state.is_final {
                let new_from = map_self[&(state as *const _)];
                for &init in &self.initial_states {
                    for &trans in init.get_transitions().iter() {
                        let new_to = map_self[&(trans.next_state as *const _)];
                        new_aut.add_transition(new_from, trans.label.clone(), new_to);
                    }
                }
            }
        }

        // Set the new automaton's initial states to be the copies of self's initial states.
        for &init in &self.initial_states {
            new_aut.initial_states.push(map_self[&(init as *const _)]);
        }

        new_aut
    }
}

/// Union construction for automata.
///
/// Each union state is represented as a pair `(Option<s>, Option<t>)`, where:
/// - `Some(s)` means automaton A is active,
/// - `Some(t)` means automaton B is active,
/// - A missing component (`None`) indicates that automaton has “fallen” into a sink.
///
/// Transition rules:
/// - If both automata can move on label `a`, add only synchronous transitions:
///      ((Some(s), Some(t)), a, (Some(s'), Some(t')))
/// - If only automaton A can move, add a transition:
///      ((Some(s), Some(t)), a, (Some(s'), None))
///   and in a sink state (Some(s), None), add self-loops driven by moves in A,
///   omitting transitions that would lead to (None, None).
/// - Similarly for automaton B.
/// - A state is final if at least one active component is final.
impl<'a, L: Eq + Hash + Clone + ValidLabel> Automata<'a, L> {
    pub fn union<'b>(
        automata_a: &Automata<'a, L>,
        automata_b: &Automata<'a, L>,
        new_states_arena: &'b Arena<State<'b, L>>,
        new_trans_arena: &'b Arena<Transition<'b, L>>,
    ) -> Automata<'b, L> {
        // Use the larger dimension.
        let new_dim = std::cmp::max(automata_a.dimensions, automata_b.dimensions);
        let mut new_aut = Automata::new(new_states_arena, new_trans_arena, new_dim);

        // Represent a union state as (Option<&AState>, Option<&BState>)
        type UnionKey<'a, L> = (Option<&'a State<'a, L>>, Option<&'a State<'a, L>>);

        let mut state_map: HashMap<UnionKey<'a, L>, &State<'b, L>> = HashMap::new();
        let mut worklist: VecDeque<UnionKey<'a, L>> = VecDeque::new();

        // Initial union states: Cartesian product of initial states.
        for &s in &automata_a.initial_states {
            for &t in &automata_b.initial_states {
                let key = (Some(s), Some(t));
                if let std::collections::hash_map::Entry::Vacant(e) = state_map.entry(key) {
                    let is_final = s.is_final || t.is_final;
                    let new_state = new_aut.add_state(true, is_final);
                    e.insert(new_state);
                    worklist.push_back(key);
                }
            }
        }

        // Process each union state.
        while let Some(key) = worklist.pop_front() {
            let current_state = state_map[&key];
            let mut labels = HashSet::new();
            if let Some(s) = key.0 {
                for tr in s.get_transitions().iter() {
                    labels.insert(tr.label.clone());
                }
            }
            if let Some(t) = key.1 {
                for tr in t.get_transitions().iter() {
                    labels.insert(tr.label.clone());
                }
            }

            for label in labels.into_iter() {
                let trans_a: Vec<_> = if let Some(s) = key.0 {
                    s.get_transitions()
                        .iter()
                        .filter(|tr| tr.label == label)
                        .copied()
                        .collect()
                } else {
                    vec![]
                };
                let trans_b: Vec<_> = if let Some(t) = key.1 {
                    t.get_transitions()
                        .iter()
                        .filter(|tr| tr.label == label)
                        .copied()
                        .collect()
                } else {
                    vec![]
                };

                if !trans_a.is_empty() && !trans_b.is_empty() {
                    // Both can move: add synchronous transitions.
                    for tr_a in &trans_a {
                        for tr_b in &trans_b {
                            let new_key = (Some(tr_a.next_state), Some(tr_b.next_state));
                            if let std::collections::hash_map::Entry::Vacant(e) =
                                state_map.entry(new_key)
                            {
                                let is_final = tr_a.next_state.is_final || tr_b.next_state.is_final;
                                let ns = new_aut.add_state(false, is_final);
                                e.insert(ns);
                                worklist.push_back(new_key);
                            }
                            let target = state_map[&new_key];
                            new_aut.add_transition(current_state, label.clone(), target);
                        }
                    }
                } else if !trans_a.is_empty() {
                    // Only automata A can move.
                    for tr_a in &trans_a {
                        let new_key = (Some(tr_a.next_state), None);
                        if let std::collections::hash_map::Entry::Vacant(e) =
                            state_map.entry(new_key)
                        {
                            let is_final = tr_a.next_state.is_final;
                            let ns = new_aut.add_state(false, is_final);
                            e.insert(ns);
                            worklist.push_back(new_key);
                        }
                        let target = state_map[&new_key];
                        new_aut.add_transition(current_state, label.clone(), target);
                    }
                } else if !trans_b.is_empty() {
                    // Only automata B can move.
                    for tr_b in &trans_b {
                        let new_key = (None, Some(tr_b.next_state));
                        if let std::collections::hash_map::Entry::Vacant(e) =
                            state_map.entry(new_key)
                        {
                            let is_final = tr_b.next_state.is_final;
                            let ns = new_aut.add_state(false, is_final);
                            e.insert(ns);
                            worklist.push_back(new_key);
                        }
                        let target = state_map[&new_key];
                        new_aut.add_transition(current_state, label.clone(), target);
                    }
                }
            }

            // For sink states: add self-loops driven by the active component.
            match key {
                (Some(s), None) => {
                    for tr in s.get_transitions().iter() {
                        let label = tr.label.clone();
                        let new_key = (Some(tr.next_state), None);
                        if let std::collections::hash_map::Entry::Vacant(e) =
                            state_map.entry(new_key)
                        {
                            let is_final = tr.next_state.is_final;
                            let ns = new_aut.add_state(false, is_final);
                            e.insert(ns);
                            worklist.push_back(new_key);
                        }
                        let target = state_map[&new_key];
                        new_aut.add_transition(current_state, label.clone(), target);
                    }
                }
                (None, Some(t)) => {
                    for tr in t.get_transitions().iter() {
                        let label = tr.label.clone();
                        let new_key = (None, Some(tr.next_state));
                        if let std::collections::hash_map::Entry::Vacant(e) =
                            state_map.entry(new_key)
                        {
                            let is_final = tr.next_state.is_final;
                            let ns = new_aut.add_state(false, is_final);
                            e.insert(ns);
                            worklist.push_back(new_key);
                        }
                        let target = state_map[&new_key];
                        new_aut.add_transition(current_state, label.clone(), target);
                    }
                }
                _ => {}
            }
        }
        new_aut
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        automata::NFAH,
        automata_runner::{AppendOnlySequence, NFAHRunner, SimpleAutomataRunner},
    };
    use itertools::Itertools;
    use typed_arena::Arena;

    fn accepts<'a>(nfah: &'a NFAH<'a>, words: &Vec<Vec<String>>) -> bool {
        assert_eq!(
            nfah.dimensions,
            words.len(),
            "The number of dimensions must match the number of words: {} vs {}",
            nfah.dimensions,
            words.len()
        );
        let mut input_sequences = (0..words.len())
            .map(|_| AppendOnlySequence::new())
            .collect_vec();
        let mut runner = SimpleAutomataRunner::new(
            nfah,
            input_sequences
                .iter()
                .map(|s| s.readable_view())
                .collect_vec(),
        );
        runner.insert_from_initial_states(
            input_sequences
                .iter()
                .map(|s| s.readable_view())
                .collect_vec(),
            (0..words.len()).collect_vec(),
        );
        for i in 0..words.len() {
            for c in words[i].iter() {
                input_sequences[i].append(c.clone());
                runner.consume();
            }
            input_sequences[i].close();
            runner.consume();
        }
        while runner.consume() {}

        runner
            .current_configurations
            .iter()
            .any(|c| c.current_state.is_final && c.input_sequence.iter().all(|s| s.is_empty()))
    }

    #[test]
    fn test_add_state() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 1);

        let current_state = automaton.add_state(true, false);
        assert_eq!(automaton.initial_states.len(), 1);
        assert!(automaton.initial_states.contains(&current_state));
        assert!(!current_state.is_final);
        assert_eq!(current_state.transitions.borrow().len(), 0);
    }

    #[test]
    fn test_add_transition() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 1);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, true);

        let t = automaton.add_nfah_transition(s1, "a".to_string(), 0, s2);
        assert_eq!(t.label.0, "a");
        assert_eq!(t.label.1, 0);
        assert_eq!(t.next_state, s2);

        let all_outgoing = s1.transitions.borrow();
        assert_eq!(all_outgoing.len(), 1);
        assert_eq!(all_outgoing[0].label.0, "a");
        assert_eq!(all_outgoing[0].label.1, 0);
        assert_eq!(all_outgoing[0].next_state, s2);
    }

    #[test]
    fn test_iter() {
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

        let mut iter = automaton.iter_states();
        assert_eq!(iter.next(), Some(s0));
        assert_eq!(iter.next(), Some(s1));
        assert_eq!(iter.next(), Some(s2));
        assert_eq!(iter.next(), Some(s3));
        assert_eq!(iter.next(), Some(sf));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_shortest_accepted_word_length() {
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

        assert_eq!(automaton.shortest_accepted_word_length(), 3);
        let prefixes = automaton.accepted_prefixes(3);
        assert_eq!(prefixes.len(), 2);
    }

    #[test]
    fn test_accepted_prefixes() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 2);

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

        // accepted prefixes of length 0 => the empty prefix
        let p0 = automaton.accepted_prefixes(0);
        assert_eq!(p0.len(), 1);
        assert!(p0.contains(&Vec::new()));

        // accepted prefixes of length 1 => either [("a", x0)] or [("b", x1)]
        let p1 = automaton.accepted_prefixes(1);
        println!("{:?}", p1);
        assert_eq!(p1.len(), 3);
        assert!(p1.contains(&vec![("a".to_string(), 0)]));
        assert!(p1.contains(&vec![("b".to_string(), 1)]));
        assert!(p1.contains(&vec![("c".to_string(), 0)]));
    }

    #[test]
    fn test_remove_unreachable_transitions() {
        let state_arena = Arena::new();
        let trans_arena = Arena::new();
        let mut automaton = NFAH::new(&state_arena, &trans_arena, 3);

        let s1 = automaton.add_state(true, false);
        let s2 = automaton.add_state(false, false);
        let s3 = automaton.add_state(false, true);
        let s4 = automaton.add_state(false, false);

        // s1 --( "a", x0 )--> s2
        automaton.add_nfah_transition(s1, "a".to_string(), 0, s2);
        // s2 --( "", x1 )--> s3
        automaton.add_nfah_transition(s2, "".to_string(), 1, s3);
        // s1 --( "x", x2 )--> s4  (but s4 does not lead to any final state)

        automaton.add_nfah_transition(s1, "x".to_string(), 2, s4);
        // Right now, s1->s4 is reachable from an initial state,
        // but s4 is not leading to any final => s4 is not "useful."
        // So that transition should be removed.

        automaton.remove_unreachable_transitions();

        // s1 should have only 1 transition left
        {
            let s1_trans = s1.transitions.borrow();
            assert_eq!(s1_trans.len(), 1);
            assert_eq!(s1_trans[0].label.0, "a");
            assert_eq!(s1_trans[0].label.1, 0);
        }
        // s4 transitions are empty, but that doesn't matter since
        // s4 won't even be recognized as "reachable to final"
        assert_eq!(s4.transitions.borrow().len(), 0);
    }

    #[test]
    fn test_emptiness_check() {
        use typed_arena::Arena;

        // =========== CASE 1: Non-empty automaton ===========
        // We'll have s0 (initial) --"a"--> s1 (final).
        let arena_s1 = Arena::new();
        let arena_t1 = Arena::new();
        let mut nfa1 = NFA::new(&arena_s1, &arena_t1, 0);

        let s0_1 = nfa1.add_state(true, false);
        let s1_1 = nfa1.add_state(false, true); // final
        nfa1.add_transition(s0_1, "a".to_string(), s1_1);

        assert!(!nfa1.is_empty(), "There is a path to a final state.");

        // =========== CASE 2: No final states at all ===========
        let arena_s2 = Arena::new();
        let arena_t2 = Arena::new();
        let mut nfa2 = NFA::new(&arena_s2, &arena_t2, 0);

        let s0_2 = nfa2.add_state(true, false);
        let s1_2 = nfa2.add_state(false, false);
        nfa2.add_transition(s0_2, "b".to_string(), s1_2);
        // No state is final here
        assert!(nfa2.is_empty(), "No final state => empty language.");

        // =========== CASE 3: Final but unreachable ===========
        let arena_s3 = Arena::new();
        let arena_t3 = Arena::new();
        let mut nfa3 = NFA::new(&arena_s3, &arena_t3, 0);

        let _s0_3 = nfa3.add_state(true, false);
        let _s1_3 = nfa3.add_state(false, false);
        let _s2_3 = nfa3.add_state(false, true); // final but not connected

        // s0_3 has no outgoing transitions at all. So s2_3 is unreachable.
        assert!(
            nfa3.is_empty(),
            "Final state is unreachable => empty language."
        );
    }

    #[test]
    fn test_product() {
        use typed_arena::Arena;

        // Build first NFAH: accepts "ab" or "aab" (small example)
        //
        //  States: s0 (initial), s1, s2 (final)
        //  Transitions:
        //    s0 --"a"--> s1
        //    s1 --"b"--> s2
        //    s0 --"a"--> s0  (so it can read multiple 'a's before a single 'b'?)
        //  => Accepts strings of one or more 'a' followed by a single 'b'
        let arena_s1 = Arena::new();
        let arena_t1 = Arena::new();
        let mut nfah1 = NFAH::new(&arena_s1, &arena_t1, 1);

        let s0_1 = nfah1.add_state(true, false);
        let s1_1 = nfah1.add_state(false, false);
        let s2_1 = nfah1.add_state(false, true); // final

        // transitions for nfah1
        nfah1.add_transition(s0_1, ("a".to_string(), 0), s1_1);
        nfah1.add_transition(s1_1, ("b".to_string(), 0), s2_1);
        // optional loop to allow multiple 'a's
        nfah1.add_transition(s0_1, ("a".to_string(), 0), s0_1);

        // Build second NFAH: accepts strings like "ab" or "abc"
        //
        //   States: p0 (initial), p1, p2 (final), p3 (non-final)
        //   Transitions:
        //       p0 --"a"--> p0
        //       p0 --"a"--> p1
        //       p1 --"b"--> p2 (final)
        //       p1 --"b"--> p3 (non-final)
        //       p3 --"c"--> p2 (final)
        //   => Accepts strings that start with ≥1 'a' and then have either "b" or "bc" to reach a final state.
        let arena_s2 = Arena::new();
        let arena_t2 = Arena::new();
        let mut nfah2 = NFAH::new(&arena_s2, &arena_t2, 1);

        let p0_2 = nfah2.add_state(true, false);
        let p1_2 = nfah2.add_state(false, false);
        let p2_2 = nfah2.add_state(false, true); // final
        let p3_2 = nfah2.add_state(false, false);

        // transitions for nfah2
        nfah2.add_transition(p0_2, ("a".to_string(), 0), p0_2);
        nfah2.add_transition(p0_2, ("a".to_string(), 0), p1_2);
        nfah2.add_transition(p1_2, ("b".to_string(), 0), p2_2);
        nfah2.add_transition(p1_2, ("b".to_string(), 0), p3_2);
        // transition from p3 leads to p2 on "c"
        nfah2.add_transition(p3_2, ("c".to_string(), 0), p2_2);

        // Build two product automata:
        //  - The intersection product (final state if both components are final)
        //  - The union product (final state if either component is final)
        let product_s_arena_inter = Arena::new();
        let product_t_arena_inter = Arena::new();
        let product_nfah = nfah1.product(&nfah2, &product_s_arena_inter, &product_t_arena_inter);
        assert_eq!(product_nfah.dimensions, nfah1.dimensions);

        // Both product automata should be non-empty and accept "ab" (shortest accepted word of length 2)
        assert!(
            !product_nfah.is_empty(),
            "Intersection product should not be empty."
        );
        let length_inter = product_nfah.shortest_accepted_word_length();
        assert_eq!(
            length_inter, 2,
            "Shortest word in the intersection is 'ab' of length 2."
        );

        {
            // 'ab' must be accepted
            let words = vec![vec!["a".to_string(), "b".to_string()]];
            assert!(accepts(&product_nfah, &words));
        }

        // For further verification, count the number of final states reached in each product automaton.
        // In our example:
        //   - Under intersection semantics, only (s2, p2) is final.
        //   - Under union semantics, both (s2, p2) and (s2, p3) become final.
        fn count_final_states(nfah: &NFAH) -> usize {
            use std::collections::{HashSet, VecDeque};
            let mut visited = HashSet::new();
            let mut queue = VecDeque::new();
            for &st in &nfah.initial_states {
                queue.push_back(st);
                visited.insert(st as *const _);
            }
            let mut count = 0;
            while let Some(st) = queue.pop_front() {
                if st.is_final {
                    count += 1;
                }
                for &tr in st.transitions.borrow().iter() {
                    let nxt_ptr = tr.next_state as *const _;
                    if visited.insert(nxt_ptr) {
                        queue.push_back(tr.next_state);
                    }
                }
            }
            count
        }

        let final_count_inter = count_final_states(&product_nfah);
        assert_eq!(
            final_count_inter, 1,
            "Intersection product should have exactly one final state."
        );
    }

    #[cfg(test)]
    mod concat_tests {
        use super::*;
        use typed_arena::Arena;

        /// Test concatenation of two simple automata.
        /// Automata A accepts only "a" and automata B accepts only "b".
        /// Their concatenation should accept "ab" (i.e. shortest accepted word length is 2).
        #[test]
        fn test_concat_simple() {
            // Automata A: accepts "a"
            let arena_a_states = Arena::new();
            let arena_a_trans = Arena::new();
            let mut automata_a = Automata::<String>::new(&arena_a_states, &arena_a_trans, 0);
            let a0 = automata_a.add_state(true, false);
            let a1 = automata_a.add_state(false, true);
            automata_a.add_transition(a0, "a".to_string(), a1);

            // Automata B: accepts "b"
            let arena_b_states = Arena::new();
            let arena_b_trans = Arena::new();
            let mut automata_b = Automata::<String>::new(&arena_b_states, &arena_b_trans, 0);
            let b0 = automata_b.add_state(true, false);
            let b1 = automata_b.add_state(false, true);
            automata_b.add_transition(b0, "b".to_string(), b1);

            // Concatenation: language should be {"ab"}
            let arena_concat_states = Arena::new();
            let arena_concat_trans = Arena::new();
            let concat_aut =
                automata_a.concat(&automata_b, &arena_concat_states, &arena_concat_trans);

            // "ab" has length 2.
            assert_eq!(
                concat_aut.shortest_accepted_word_length(),
                2,
                "Concatenation of A and B should accept 'ab'"
            );
        }

        /// Test concatenation where the first automata accepts ε.
        /// Automata A accepts the empty word, and automata B accepts "b".
        /// The concatenation should accept "b" (shortest word of length 1).
        #[test]
        fn test_concat_with_epsilon_in_first() {
            // Automata A: accepts ε (one state, both initial and final)
            let arena_a_states = Arena::new();
            let arena_a_trans = Arena::new();
            let mut automata_a = Automata::<String>::new(&arena_a_states, &arena_a_trans, 0);
            automata_a.add_state(true, true);
            // No transitions

            // Automata B: accepts "b"
            let arena_b_states = Arena::new();
            let arena_b_trans = Arena::new();
            let mut automata_b = Automata::<String>::new(&arena_b_states, &arena_b_trans, 0);
            let b0 = automata_b.add_state(true, false);
            let b1 = automata_b.add_state(false, true);
            automata_b.add_transition(b0, "b".to_string(), b1);

            // Concatenation: since A accepts ε, the language should be exactly that of B.
            let arena_concat_states = Arena::new();
            let arena_concat_trans = Arena::new();
            let concat_aut =
                automata_a.concat(&automata_b, &arena_concat_states, &arena_concat_trans);

            // "b" has length 1.
            assert_eq!(
                concat_aut.shortest_accepted_word_length(),
                1,
                "Concatenation when A accepts ε should yield language of B"
            );
        }

        /// Test concatenation where the second automata accepts ε.
        /// Automata A accepts "a", and automata B accepts ε.
        /// In this case a run can finish in A (if B does nothing) because B accepts ε.
        /// The concatenation should accept "a" (shortest accepted word length is 1).
        #[test]
        fn test_concat_with_epsilon_in_second() {
            // Automata A: accepts "a"
            let arena_a_states = Arena::new();
            let arena_a_trans = Arena::new();
            let mut automata_a = Automata::<String>::new(&arena_a_states, &arena_a_trans, 0);
            let a0 = automata_a.add_state(true, false);
            let a1 = automata_a.add_state(false, true);
            automata_a.add_transition(a0, "a".to_string(), a1);

            // Automata B: accepts ε (one state, both initial and final)
            let arena_b_states = Arena::new();
            let arena_b_trans = Arena::new();
            let mut automata_b = Automata::<String>::new(&arena_b_states, &arena_b_trans, 0);
            automata_b.add_state(true, true);
            // No transitions

            // Concatenation: since B accepts ε, a run ending in A's final state should be accepted.
            let arena_concat_states = Arena::new();
            let arena_concat_trans = Arena::new();
            let concat_aut =
                automata_a.concat(&automata_b, &arena_concat_states, &arena_concat_trans);

            // "a" has length 1.
            assert_eq!(
            concat_aut.shortest_accepted_word_length(),
            1,
            "Concatenation when B accepts ε should yield language that includes A's accepted words"
        );
        }

        /// Test concatenation where both automata accept ε.
        /// Then the concatenated language should include the empty word.
        #[test]
        fn test_concat_epsilon_epsilon() {
            // Automata A: accepts ε
            let arena_a_states = Arena::new();
            let arena_a_trans = Arena::new();
            let mut automata_a = Automata::<String>::new(&arena_a_states, &arena_a_trans, 0);
            automata_a.add_state(true, true);

            // Automata B: accepts ε
            let arena_b_states = Arena::new();
            let arena_b_trans = Arena::new();
            let mut automata_b = Automata::<String>::new(&arena_b_states, &arena_b_trans, 0);
            automata_b.add_state(true, true);

            // Concatenation: should accept ε (shortest accepted word length is 0)
            let arena_concat_states = Arena::new();
            let arena_concat_trans = Arena::new();
            let concat_aut =
                automata_a.concat(&automata_b, &arena_concat_states, &arena_concat_trans);

            assert_eq!(
                concat_aut.shortest_accepted_word_length(),
                0,
                "Concatenation of two ε-accepting automata should accept ε"
            );
        }
    }

    #[test]
    fn test_accepts_nfah_multiple_dimensions() {
        // Build an NFAH for 2 dimensions that accepts if the first input is "a" and the second is "b"
        let arena_states = Arena::new();
        let arena_trans = Arena::new();
        let mut automata = NFAH::new(&arena_states, &arena_trans, 2);
        let s0 = automata.add_state(true, false);
        let s1 = automata.add_state(false, false);
        let s2 = automata.add_state(false, true);
        automata.add_transition(s0, ("a".to_string(), 0), s1);
        automata.add_transition(s1, ("b".to_string(), 1), s2);
        let accepted = accepts(
            &automata,
            &vec![vec!["a".to_string()], vec!["b".to_string()]],
        );
        assert!(
            accepted,
            "The automata should accept the input ['a'], ['b']"
        );
        let rejected = accepts(
            &automata,
            &vec![vec!["a".to_string()], vec!["c".to_string()]],
        );
        assert!(
            !rejected,
            "The automata should not accept the input ['a'], ['c']"
        );
    }

    #[cfg(test)]
    mod star_plus_tests {
        use super::*;
        use typed_arena::Arena;

        /// Test that the Kleene star of an automaton accepting "a" accepts ε.
        #[test]
        fn test_star_accepts_empty() {
            // Build an automaton that accepts "a"
            let arena_states = Arena::new();
            let arena_trans = Arena::new();
            let mut automata = Automata::<String>::new(&arena_states, &arena_trans, 0);
            let s0 = automata.add_state(true, false);
            let s1 = automata.add_state(false, true);
            automata.add_transition(s0, "a".to_string(), s1);

            // Build the Kleene star of automata.
            let arena_star_states = Arena::new();
            let arena_star_trans = Arena::new();
            let star_aut = automata.star(&arena_star_states, &arena_star_trans);

            // Since star should always accept ε, the shortest accepted word length is 0.
            assert_eq!(
                star_aut.shortest_accepted_word_length(),
                0,
                "Kleene star should accept the empty word"
            );

            // Additionally, check that non-empty words are accepted.
            // Accepted prefixes of length 1 should include "a".
            let prefixes_len1 = star_aut.accepted_prefixes(1);
            assert!(
                prefixes_len1.contains(&vec!["a".to_string()]),
                "Kleene star should accept 'a'"
            );
        }

        /// Test that the Kleene star of an automaton accepting "a" also accepts repeated occurrences.
        #[test]
        fn test_star_multiple_occurrences() {
            // Build an automaton that accepts "a"
            let arena_states = Arena::new();
            let arena_trans = Arena::new();
            let mut automata = Automata::<String>::new(&arena_states, &arena_trans, 0);
            let s0 = automata.add_state(true, false);
            let s1 = automata.add_state(false, true);
            automata.add_transition(s0, "a".to_string(), s1);

            // Build the Kleene star of automata.
            let arena_star_states = Arena::new();
            let arena_star_trans = Arena::new();
            let star_aut = automata.star(&arena_star_states, &arena_star_trans);

            // Accepted prefixes of length 2 should include "aa".
            let prefixes_len2 = star_aut.accepted_prefixes(2);
            assert!(
                prefixes_len2.contains(&vec!["a".to_string(), "a".to_string()]),
                "Kleene star should accept 'aa'"
            );
        }

        /// Test that the Kleene plus of an automaton accepting "a" does not accept ε.
        #[test]
        fn test_plus_no_epsilon() {
            // Build an automaton that accepts "a"
            let arena_states = Arena::new();
            let arena_trans = Arena::new();
            let mut automata = Automata::<String>::new(&arena_states, &arena_trans, 0);
            let s0 = automata.add_state(true, false);
            let s1 = automata.add_state(false, true);
            automata.add_transition(s0, "a".to_string(), s1);

            // Build the Kleene plus of automata.
            let arena_plus_states = Arena::new();
            let arena_plus_trans = Arena::new();
            let plus_aut = automata.plus(&arena_plus_states, &arena_plus_trans);

            // Since plus should require at least one occurrence, the shortest accepted word is "a" (length 1).
            assert_eq!(
                plus_aut.shortest_accepted_word_length(),
                1,
                "Kleene plus should not accept the empty word if the original automata doesn't"
            );

            // Check that accepted prefixes of length 1 include "a".
            let prefixes_len1 = plus_aut.accepted_prefixes(1);
            assert!(
                prefixes_len1.contains(&vec!["a".to_string()]),
                "Kleene plus should accept 'a'"
            );
        }

        /// Test that the Kleene plus of an automaton accepting "a" also accepts repeated occurrences.
        #[test]
        fn test_plus_multiple_occurrences() {
            // Build an automaton that accepts "a"
            let arena_states = Arena::new();
            let arena_trans = Arena::new();
            let mut automata = Automata::<String>::new(&arena_states, &arena_trans, 0);
            let s0 = automata.add_state(true, false);
            let s1 = automata.add_state(false, true);
            automata.add_transition(s0, "a".to_string(), s1);

            // Build the Kleene plus of automata.
            let arena_plus_states = Arena::new();
            let arena_plus_trans = Arena::new();
            let plus_aut = automata.plus(&arena_plus_states, &arena_plus_trans);

            // Accepted prefixes of length 2 should include "aa".
            let prefixes_len2 = plus_aut.accepted_prefixes(2);
            assert!(
                prefixes_len2.contains(&vec!["a".to_string(), "a".to_string()]),
                "Kleene plus should accept 'aa'"
            );
        }

        /// Test that the Kleene plus of an automaton that already accepts ε continues to accept ε.
        #[test]
        fn test_plus_accepts_epsilon_when_original_accepts_epsilon() {
            // Build an automaton that accepts ε (one state, both initial and final)
            let arena_states = Arena::new();
            let arena_trans = Arena::new();
            let mut automata = Automata::<String>::new(&arena_states, &arena_trans, 0);
            automata.add_state(true, true);
            // No transitions

            // Build the Kleene plus of automata.
            let arena_plus_states = Arena::new();
            let arena_plus_trans = Arena::new();
            let plus_aut = automata.plus(&arena_plus_states, &arena_plus_trans);

            // Since the original automata accepts ε, plus should also accept ε.
            assert_eq!(
                plus_aut.shortest_accepted_word_length(),
                0,
                "Kleene plus should accept ε if the original automata accepts ε"
            );
        }

        #[test]
        fn test_star_accepts_words() {
            // Build an NFAH that accepts the word "a"
            let arena_states = Arena::new();
            let arena_trans = Arena::new();
            let mut automata = NFAH::new(&arena_states, &arena_trans, 1);
            let s0 = automata.add_state(true, false);
            let s1 = automata.add_state(false, true);
            automata.add_transition(s0, ("a".to_string(), 0), s1);
            // Build the Kleene star of the automata.
            let arena_star_states = Arena::new();
            let arena_star_trans = Arena::new();
            let star_aut = automata.star(&arena_star_states, &arena_star_trans);
            assert!(
                accepts(&star_aut, &vec![vec![]]),
                "Kleene star should accept empty word"
            );
            assert!(
                accepts(&star_aut, &vec![vec!["a".to_string()]]),
                "Kleene star should accept 'a'"
            );
            assert!(
                accepts(&star_aut, &vec![vec!["a".to_string(), "a".to_string()]]),
                "Kleene star should accept 'aa'"
            );
        }

        #[test]
        fn test_plus_accepts_words() {
            // Build an NFAH that accepts the word "a"
            let arena_states = Arena::new();
            let arena_trans = Arena::new();
            let mut automata = NFAH::new(&arena_states, &arena_trans, 1);
            let s0 = automata.add_state(true, false);
            let s1 = automata.add_state(false, true);
            automata.add_transition(s0, ("a".to_string(), 0), s1);
            // Build the Kleene plus of the automata.
            let arena_plus_states = Arena::new();
            let arena_plus_trans = Arena::new();
            let plus_aut = automata.plus(&arena_plus_states, &arena_plus_trans);
            assert!(
                !accepts(&plus_aut, &vec![vec![]]),
                "Kleene plus should not accept empty word"
            );
            assert!(
                accepts(&plus_aut, &vec![vec!["a".to_string()]]),
                "Kleene plus should accept 'a'"
            );
            assert!(
                accepts(&plus_aut, &vec![vec!["a".to_string(), "a".to_string()]]),
                "Kleene plus should accept 'aa'"
            );
        }
    }

    #[cfg(test)]
    mod union_tests {
        use super::*;
        use std::collections::HashSet;
        use typed_arena::Arena;

        // Helper: build a simple automaton that accepts a single letter.
        fn build_single_letter_automata<'a>(
            state_arena: &'a Arena<State<'a, String>>,
            trans_arena: &'a Arena<Transition<'a, String>>,
            letter: &'a str,
            is_final: bool,
        ) -> Automata<'a, String> {
            let mut aut = Automata::new(state_arena, trans_arena, 0);
            let s0 = aut.add_state(true, false);
            let s1 = aut.add_state(false, is_final);
            aut.add_transition(s0, letter.to_string(), s1);
            aut
        }

        #[test]
        fn test_union_single_letter() {
            let state_arena = Arena::new();
            let trans_arena = Arena::new();

            // Automata A accepts "a"
            let aut_a = build_single_letter_automata(&state_arena, &trans_arena, "a", true);
            // Automata B accepts "b"
            let aut_b = build_single_letter_automata(&state_arena, &trans_arena, "b", true);
            let union_state_arena = Arena::new();
            let union_trans_arena = Arena::new();
            let union_aut = Automata::union(&aut_a, &aut_b, &union_state_arena, &union_trans_arena);
            // Expect union automaton to accept "a" and "b" (shortest word = 1)
            assert_eq!(union_aut.shortest_accepted_word_length(), 1);
            let prefixes = union_aut.accepted_prefixes(1);
            let expected: HashSet<Vec<String>> = vec![vec!["a".to_string()], vec!["b".to_string()]]
                .into_iter()
                .collect();
            assert_eq!(prefixes, expected);
        }

        #[test]
        fn test_union_synchronous() {
            // Automata A: s0 -- "a" --> s1 (s1 final)
            let state_arena_a = Arena::new();
            let trans_arena_a = Arena::new();
            let mut aut_a = Automata::new(&state_arena_a, &trans_arena_a, 0);
            let s0_a = aut_a.add_state(true, false);
            let s1_a = aut_a.add_state(false, true);
            aut_a.add_transition(s0_a, "a".to_string(), s1_a);

            // Automata B: t0 -- "a" --> t1 (t1 final)
            let state_arena_b = Arena::new();
            let trans_arena_b = Arena::new();
            let mut aut_b = Automata::new(&state_arena_b, &trans_arena_b, 0);
            let t0_b = aut_b.add_state(true, false);
            let t1_b = aut_b.add_state(false, true);
            aut_b.add_transition(t0_b, "a".to_string(), t1_b);

            let union_state_arena = Arena::new();
            let union_trans_arena = Arena::new();
            let union_aut = Automata::union(&aut_a, &aut_b, &union_state_arena, &union_trans_arena);
            // For "a", should use synchronous move to (Some(s1), Some(t1))
            assert_eq!(union_aut.shortest_accepted_word_length(), 1);
            let prefixes = union_aut.accepted_prefixes(1);
            let expected: HashSet<Vec<String>> = vec![vec!["a".to_string()]].into_iter().collect();
            assert_eq!(prefixes, expected);
        }

        #[test]
        fn test_union_asynchronous_a() {
            // Automata A: s0 -- "a" --> s1 (s1 final)
            let state_arena_a = Arena::new();
            let trans_arena_a = Arena::new();
            let mut aut_a = Automata::new(&state_arena_a, &trans_arena_a, 0);
            let s0_a = aut_a.add_state(true, false);
            let s1_a = aut_a.add_state(false, true);
            aut_a.add_transition(s0_a, "a".to_string(), s1_a);

            // Automata B: t0 with no "a" transition.
            let state_arena_b = Arena::new();
            let trans_arena_b = Arena::new();
            let mut aut_b = Automata::new(&state_arena_b, &trans_arena_b, 0);
            let _t0_b = aut_b.add_state(true, false);

            let union_state_arena = Arena::new();
            let union_trans_arena = Arena::new();
            let union_aut = Automata::union(&aut_a, &aut_b, &union_state_arena, &union_trans_arena);
            // "a" should move asynchronously: (Some(s0), Some(t0)) -- "a" --> (Some(s1), None)
            assert_eq!(union_aut.shortest_accepted_word_length(), 1);
            let prefixes = union_aut.accepted_prefixes(1);
            let expected: HashSet<Vec<String>> = vec![vec!["a".to_string()]].into_iter().collect();
            assert_eq!(prefixes, expected);
        }

        #[test]
        fn test_union_asynchronous_b() {
            // Automata A: s0 with no "a" transition.
            let state_arena_a = Arena::new();
            let trans_arena_a = Arena::new();
            let mut aut_a = Automata::new(&state_arena_a, &trans_arena_a, 0);
            let _s0_a = aut_a.add_state(true, false);

            // Automata B: t0 -- "a" --> t1 (t1 final)
            let state_arena_b = Arena::new();
            let trans_arena_b = Arena::new();
            let mut aut_b = Automata::new(&state_arena_b, &trans_arena_b, 0);
            let t0_b = aut_b.add_state(true, false);
            let t1_b = aut_b.add_state(false, true);
            aut_b.add_transition(t0_b, "a".to_string(), t1_b);

            let union_state_arena = Arena::new();
            let union_trans_arena = Arena::new();
            let union_aut = Automata::union(&aut_a, &aut_b, &union_state_arena, &union_trans_arena);
            // "a" should move asynchronously: (Some(s0), Some(t0)) -- "a" --> (None, Some(t1))
            assert_eq!(union_aut.shortest_accepted_word_length(), 1);
            let prefixes = union_aut.accepted_prefixes(1);
            let expected: HashSet<Vec<String>> = vec![vec!["a".to_string()]].into_iter().collect();
            assert_eq!(prefixes, expected);
        }

        #[test]
        fn test_union_sink_self_loop() {
            // Automata A: s0 -- "a" --> s1, s1 -- "b" --> s2 (s2 final)
            let state_arena_a = Arena::new();
            let trans_arena_a = Arena::new();
            let mut aut_a = Automata::new(&state_arena_a, &trans_arena_a, 0);
            let s0_a = aut_a.add_state(true, false);
            let s1_a = aut_a.add_state(false, false);
            let s2_a = aut_a.add_state(false, true);
            aut_a.add_transition(s0_a, "a".to_string(), s1_a);
            aut_a.add_transition(s1_a, "b".to_string(), s2_a);

            // Automata B: t0 with no transitions.
            let state_arena_b = Arena::new();
            let trans_arena_b = Arena::new();
            let mut aut_b = Automata::new(&state_arena_b, &trans_arena_b, 0);
            let _t0_b = aut_b.add_state(true, false);

            let union_state_arena = Arena::new();
            let union_trans_arena = Arena::new();
            let union_aut = Automata::union(&aut_a, &aut_b, &union_state_arena, &union_trans_arena);
            // Expect "ab" accepted: (Some(s0), Some(t0)) -- "a" --> (Some(s1), None)
            // then (Some(s1), None) -- "b" --> (Some(s2), None)
            assert_eq!(union_aut.shortest_accepted_word_length(), 2);
            let prefixes = union_aut.accepted_prefixes(2);
            let expected: HashSet<Vec<String>> = vec![vec!["a".to_string(), "b".to_string()]]
                .into_iter()
                .collect();
            assert_eq!(prefixes, expected);
        }

        #[test]
        fn test_union_empty_word_acceptance() {
            // Automata A: accepts empty word (s0 initial & final)
            let state_arena_a = Arena::new();
            let trans_arena_a = Arena::new();
            let mut aut_a = Automata::new(&state_arena_a, &trans_arena_a, 0);
            let _s0_a = aut_a.add_state(true, true);

            // Automata B: t0 -- "b" --> t1 (t1 final)
            let state_arena_b = Arena::new();
            let trans_arena_b = Arena::new();
            let mut aut_b = Automata::new(&state_arena_b, &trans_arena_b, 0);
            let t0_b = aut_b.add_state(true, false);
            let _t1_b = aut_b.add_state(false, true);
            aut_b.add_transition(t0_b, "b".to_string(), _t1_b);

            let union_state_arena = Arena::new();
            let union_trans_arena = Arena::new();
            let union_aut = Automata::union(&aut_a, &aut_b, &union_state_arena, &union_trans_arena);
            // Because A accepts ε, union should accept the empty word.
            assert_eq!(union_aut.shortest_accepted_word_length(), 0);
        }

        #[test]
        fn test_union_cycle_handling() {
            // Automata A: s0 -- "a" --> s1, s1 -- "a" --> s1 (s1 final)
            let state_arena_a = Arena::new();
            let trans_arena_a = Arena::new();
            let mut aut_a = Automata::new(&state_arena_a, &trans_arena_a, 0);
            let s0_a = aut_a.add_state(true, false);
            let s1_a = aut_a.add_state(false, true);
            aut_a.add_transition(s0_a, "a".to_string(), s1_a);
            aut_a.add_transition(s1_a, "a".to_string(), s1_a);

            // Automata B: t0 -- "a" --> t0 (non-final)
            let state_arena_b = Arena::new();
            let trans_arena_b = Arena::new();
            let mut aut_b = Automata::new(&state_arena_b, &trans_arena_b, 0);
            let t0_b = aut_b.add_state(true, false);
            aut_b.add_transition(t0_b, "a".to_string(), t0_b);

            let union_state_arena = Arena::new();
            let union_trans_arena = Arena::new();
            let union_aut = Automata::union(&aut_a, &aut_b, &union_state_arena, &union_trans_arena);
            // Expected shortest accepted word is "a" (from (Some(s0), Some(t0)) on "a" to (Some(s1), Some(t0)))
            assert_eq!(union_aut.shortest_accepted_word_length(), 1);
        }

        #[test]
        fn test_union_multiple_transitions() {
            // Automata A: s0 with two transitions on "a": one to s1 (final) and one to s2 (non-final)
            let state_arena_a = Arena::new();
            let trans_arena_a = Arena::new();
            let mut aut_a = Automata::new(&state_arena_a, &trans_arena_a, 0);
            let s0_a = aut_a.add_state(true, false);
            let s1_a = aut_a.add_state(false, true);
            let s2_a = aut_a.add_state(false, false);
            aut_a.add_transition(s0_a, "a".to_string(), s1_a);
            aut_a.add_transition(s0_a, "a".to_string(), s2_a);

            // Automata B: t0 -- "a" --> t1.
            let state_arena_b = Arena::new();
            let trans_arena_b = Arena::new();
            let mut aut_b = Automata::new(&state_arena_b, &trans_arena_b, 0);
            let t0_b = aut_b.add_state(true, false);
            let t1_b = aut_b.add_state(false, true);
            aut_b.add_transition(t0_b, "a".to_string(), t1_b);

            let union_state_arena = Arena::new();
            let union_trans_arena = Arena::new();
            let union_aut = Automata::union(&aut_a, &aut_b, &union_state_arena, &union_trans_arena);
            // Expected shortest accepted word is "a" because one branch (s1 final) accepts.
            assert_eq!(union_aut.shortest_accepted_word_length(), 1);
        }

        #[test]
        fn test_union_dead_states() {
            // Automata A: s0 with no outgoing transitions.
            let state_arena_a = Arena::new();
            let trans_arena_a = Arena::new();
            let mut aut_a = Automata::new(&state_arena_a, &trans_arena_a, 0);
            let _s0_a = aut_a.add_state(true, false);

            // Automata B: t0 -- "a" --> t1 (t1 final)
            let state_arena_b = Arena::new();
            let trans_arena_b = Arena::new();
            let mut aut_b = Automata::new(&state_arena_b, &trans_arena_b, 0);
            let t0_b = aut_b.add_state(true, false);
            let _t1_b = aut_b.add_state(false, true);
            aut_b.add_transition(t0_b, "a".to_string(), _t1_b);

            let union_state_arena = Arena::new();
            let union_trans_arena = Arena::new();
            let union_aut = Automata::union(&aut_a, &aut_b, &union_state_arena, &union_trans_arena);
            // Expect that "a" is accepted via an asynchronous move from B,
            // and no transitions lead to (None, None).
            assert_eq!(union_aut.shortest_accepted_word_length(), 1);
        }
    }
}
