use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use log::{debug, info, trace};

use crate::automata::NFAH;

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

pub struct QuickSearchSkipValues {
    /// Returns the length of the shortest accepted word projected to each variable.
    pub shortest_accepted_word_length_map: Vec<usize>,
    pub last_accepted_word: Vec<HashSet<String>>,
    skip_values_map: Vec<HashMap<String, usize>>,
}

impl QuickSearchSkipValues {
    /// Creates a new `QuickSearchSkipValues` instance.
    ///
    /// # Returns
    ///
    /// A new `QuickSearchSkipValues` instance.
    pub fn new(autom: &NFAH) -> Self {
        // Start measuring the time it takes to construct the skip value table
        let start = Instant::now();

        let shortest_length = autom.shortest_accepted_word_length();
        let accepted_prefixes = autom.accepted_prefixes(shortest_length);
        let shortest_accepted_word_length_map: Vec<usize> = (0..autom.dimensions)
            .map(|var| {
                accepted_prefixes
                    .iter()
                    .map(|prefix| prefix.project(var).len())
                    .min()
                    .unwrap_or(0)
            })
            .collect();
        let accepted_words: Vec<Vec<Vec<String>>> = (0..autom.dimensions)
            .map(|var| {
                accepted_prefixes
                    .iter()
                    .map(|prefix| prefix.project(var).clone())
                    .collect()
            })
            .collect();

        let last_accepted_word = (0..autom.dimensions)
            .map(|var| {
                if shortest_accepted_word_length_map[var] == 0 {
                    HashSet::new()
                } else {
                    let ind = shortest_accepted_word_length_map[var] - 1;
                    accepted_words[var]
                        .iter()
                        .map(|word| word[ind].clone())
                        .collect()
                }
            })
            .collect();

        let mut skip_values_map = Vec::with_capacity(autom.dimensions);
        for var in 0..autom.dimensions {
            let mut skip_values = HashMap::new();
            let shortest_accepted_word_length = shortest_accepted_word_length_map[var];
            for word in accepted_words[var].iter() {
                for i in 0..shortest_accepted_word_length {
                    let key = &word[shortest_accepted_word_length - 1 - i];
                    if !skip_values.contains_key(key) {
                        skip_values.insert(key.clone(), i + 1);
                        break;
                    } else if skip_values[key] > i + 1 {
                        *skip_values.get_mut(key).unwrap() = i + 1;
                    }
                }
            }
            skip_values_map.push(skip_values);
        }

        let duration = start.elapsed();
        info!(
            "Constructed Quick-Search-style skip value table (Time elapsed: {:?})",
            duration
        );
        debug!("shortest_accepted_word_length: {:?}", shortest_accepted_word_length_map);
        debug!("last_accepted_word_length: {:?}", last_accepted_word);
        debug!("skip_values_map: {:?}", skip_values_map);
        debug!("Note: The skip value is shortest_accepted_word_length + 1 for letters not in the above map.");
        QuickSearchSkipValues {
            shortest_accepted_word_length_map,
            last_accepted_word,
            skip_values_map,
        }
    }

    pub fn skip_value(&self, action: &str, variable: usize) -> usize {
        if variable >= self.skip_values_map.len() {
            panic!("Variable index out of bounds");
        }
        if self.skip_values_map[variable].contains_key(action) {
            self.skip_values_map[variable][action]
        } else {
            self.shortest_accepted_word_length_map[variable] + 1
        }
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

        let quick_search_skip_values = QuickSearchSkipValues::new(&automaton);
        assert_eq!(
            quick_search_skip_values.shortest_accepted_word_length_map,
            vec![2, 1]
        );
        assert_eq!(quick_search_skip_values.last_accepted_word[0].len(), 2);
        assert!(quick_search_skip_values.last_accepted_word[0].contains("a"));
        assert!(quick_search_skip_values.last_accepted_word[0].contains("c"));
        assert_eq!(quick_search_skip_values.last_accepted_word[1].len(), 1);
        assert!(quick_search_skip_values.last_accepted_word[1].contains("c"));

        assert_eq!(quick_search_skip_values.skip_value("a", 0), 1);
        assert_eq!(quick_search_skip_values.skip_value("b", 0), 3);
        assert_eq!(quick_search_skip_values.skip_value("c", 0), 1);
        assert_eq!(quick_search_skip_values.skip_value("a", 1), 2);
        assert_eq!(quick_search_skip_values.skip_value("b", 1), 2);
        assert_eq!(quick_search_skip_values.skip_value("c", 1), 1);
    }
}
