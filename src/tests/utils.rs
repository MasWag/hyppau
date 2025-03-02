use crate::result_notifier::{MatchingInterval, MatchingResult};

/// Helper function to verify matching results against expected intervals
pub fn verify_intervals(results: &[MatchingResult], expected_intervals: &[Vec<usize>]) {
    assert_eq!(
        results.len(),
        expected_intervals.len(),
        "Number of results doesn't match expected"
    );

    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.intervals.len(), 2, "Result should have 2 intervals");
        assert_eq!(
            result.intervals[0],
            MatchingInterval::new(expected_intervals[i][0], expected_intervals[i][1]),
            "First interval mismatch at result {}",
            i
        );
        assert_eq!(
            result.intervals[1],
            MatchingInterval::new(expected_intervals[i][2], expected_intervals[i][3]),
            "Second interval mismatch at result {}",
            i
        );
    }
}

/// Helper function to verify matching results against expected ids
pub fn verify_ids(results: &[MatchingResult], expected_ids: &[Vec<usize>]) {
    assert_eq!(
        results.len(),
        expected_ids.len(),
        "Number of results doesn't match expected"
    );

    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.ids.len(), 2, "Result should have 2 IDs");
        assert_eq!(
            result.ids[0], expected_ids[i][0],
            "First ID mismatch at result {}",
            i
        );
        assert_eq!(
            result.ids[1], expected_ids[i][1],
            "Second ID mismatch at result {}",
            i
        );
    }
}
