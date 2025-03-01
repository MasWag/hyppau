use clap::{ArgAction, Parser, ValueEnum};
use env_logger::Env;
use filtered_single_hyper_pattern_matching::NaiveFilteredSingleHyperPatternMatching;
use log::{debug, error, info, trace};
use std::fs::File;
use std::io::{BufReader, Read};
use typed_arena::Arena;

use crate::automata_runner::AppendOnlySequence;
use crate::multi_stream_reader::{MultiStreamReader, StreamSource};
use crate::result_notifier::{
    FileResultNotifier, MatchingInterval, ResultNotifier, StdoutResultNotifier,
};
use crate::serialization::{automaton_to_dot, deserialize_nfa};

#[derive(Clone)]
enum ResultNotifierType {
    Stdout(StdoutResultNotifier),
    File(FileResultNotifier),
}

impl ResultNotifier for ResultNotifierType {
    fn notify(&mut self, intervals: &[MatchingInterval], ids: &[usize]) {
        match self {
            ResultNotifierType::Stdout(notifier) => notifier.notify(intervals, ids),
            ResultNotifierType::File(notifier) => notifier.notify(intervals, ids),
        }
    }
}

use crate::hyper_pattern_matching::OnlineHyperPatternMatching;
use crate::naive_hyper_pattern_matching::NaiveHyperPatternMatching;
use crate::reading_scheduler::ReadingScheduler;

#[derive(Clone, Debug, ValueEnum)]
enum Mode {
    Naive,
    Online,
    Fjs,
    NaiveFiltered,
    OnlineFiltered,
}

/// A prototype tool for Hyper Pattern Matching
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Read an automaton written in JSON format from FILE.
    #[arg(short = 'f', long = "automaton", value_name = "FILE")]
    automaton: String,

    /// Read the log from FILE (can be used multiple times).
    #[arg(short = 'i', long = "input", value_name = "FILE")]
    input: Vec<String>,

    /// Quiet mode. Causes any results to be suppressed.
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Print the automaton in Graphviz DOT format.
    #[arg(short = 'g', long = "graphviz")]
    graphviz: bool,

    /// Write the output to FILE instead of stdout.
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    output: Option<String>,

    /// Verbose mode. Use -v for debug messages and -vv for trace messages.
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    verbose: u8,

    /// Choose the matching mode: naive or online (default: naive)
    #[arg(short = 'm', long = "mode", value_enum, default_value_t = Mode::Naive)]
    mode: Mode,
}

mod automata;
mod automata_runner;
mod dfa;
mod dfa_earliest_pattern_matcher;
mod filtered_hyper_pattern_matching;
mod filtered_pattern_matching_automata_runner;
mod filtered_single_hyper_pattern_matching;
mod fjs_hyper_pattern_matching;
mod hyper_pattern_matching;
mod kmp_skip_values;
mod matching_filter;
mod multi_stream_reader;
mod naive_hyper_pattern_matching;
mod nfa;
mod nfah;
mod online_single_hyper_pattern_matching;
mod quick_search_skip_values;
mod reading_scheduler;
mod result_notifier;
mod serialization;
mod shared_buffer;
mod single_hyper_pattern_matching;
#[cfg(test)]
mod tests;

fn main() {
    // Parse the command-line arguments
    let args = Args::parse();

    // Determine log level based on quiet flag and the number of -v occurrences.
    let log_level = if args.quiet {
        "warn"
    } else if args.verbose >= 2 {
        "trace"
    } else if args.verbose == 1 {
        "debug"
    } else {
        "info"
    };

    // Set up the default log level based on the computed log level unless overridden by RUST_LOG.
    let env = Env::default().filter_or("RUST_LOG", log_level);
    env_logger::Builder::from_env(env).init();

    // Log the parsed arguments for debugging
    trace!("Parsed command-line arguments: {:?}", args);

    // Log status messages
    debug!("Automaton file: {}", args.automaton);
    if !args.input.is_empty() {
        debug!("Input file(s): {:?}", args.input);
    }
    debug!("Quiet mode: {}", args.quiet);
    debug!("Graphviz output: {}", args.graphviz);
    debug!("Matching mode: {:?}", args.mode);

    // Read the automaton file
    let mut file = match File::open(&args.automaton) {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open automaton file: {}", e);
            return;
        }
    };

    let mut contents = String::new();
    if let Err(e) = file.read_to_string(&mut contents) {
        error!("Failed to read automaton file: {}", e);
        return;
    }

    // Create arenas for states and transitions
    let state_arena = Arena::new();
    let trans_arena = Arena::new();

    // Deserialize the JSON content into an automaton
    let automaton = deserialize_nfa(&contents, &state_arena, &trans_arena);

    // Print some information about the constructed automaton
    debug!("Automaton constructed successfully");
    debug!("Number of states: {}", automaton.states.len());
    debug!(
        "Number of initial states: {}",
        automaton.initial_states.len()
    );
    debug!("Number of dimensions: {}", automaton.dimensions);

    // If the --graphviz option is used, generate the automaton in DOT format
    if args.graphviz {
        let dot_output = automaton_to_dot(&automaton);

        // If an output file is specified, write to the file; otherwise, print to stdout
        if let Some(output_file) = args.output {
            match std::fs::write(&output_file, dot_output) {
                Ok(_) => info!("DOT output written to file: {}", output_file),
                Err(e) => error!("Failed to write DOT output to file: {}", e),
            }
        } else {
            println!("{}", dot_output);
        }
        return;
    }
    // If no input files are specified, print a message and return
    if args.input.is_empty() {
        info!("No input files specified; nothing to do");
        return;
    }

    // Construct MultiStreamReader from the input files
    debug!(
        "Construct MultiStreamReader from input files: {:?}",
        args.input
    );
    let multi_stream_reader = MultiStreamReader::new(
        args.input
            .iter()
            .map(|path| {
                let file = std::fs::File::open(path).unwrap();
                Box::new(BufReader::new(file)) as Box<dyn StreamSource>
            })
            .collect(),
    );

    // Construct ResultNotifier
    let result_notifier = if let Some(output_file) = args.output {
        ResultNotifierType::File(FileResultNotifier::new(&output_file).unwrap())
    } else {
        ResultNotifierType::Stdout(StdoutResultNotifier)
    };

    // Construct HyperPatternMatching and ReadingScheduler depending on the mode argument
    info!("Start hyper pattern matching with {:?} mode", args.mode);
    match args.mode {
        Mode::Naive => {
            let hyper_pattern_matching = NaiveHyperPatternMatching::new(
                &automaton,
                result_notifier,
                args.input
                    .into_iter()
                    .map(|_| AppendOnlySequence::new())
                    .collect(),
            );
            let mut reading_scheduler =
                ReadingScheduler::new(hyper_pattern_matching, multi_stream_reader);
            reading_scheduler.run();
        }
        Mode::Online => {
            let hyper_pattern_matching = OnlineHyperPatternMatching::new(
                &automaton,
                result_notifier,
                args.input
                    .into_iter()
                    .map(|_| AppendOnlySequence::new())
                    .collect(),
            );
            let mut reading_scheduler =
                ReadingScheduler::new(hyper_pattern_matching, multi_stream_reader);
            reading_scheduler.run();
        }
        Mode::Fjs => {
            use crate::fjs_hyper_pattern_matching::FJSHyperPatternMatching;
            let hyper_pattern_matching = FJSHyperPatternMatching::new(
                &automaton,
                result_notifier,
                args.input
                    .into_iter()
                    .map(|_| AppendOnlySequence::new())
                    .collect(),
            );
            let mut reading_scheduler =
                ReadingScheduler::new(hyper_pattern_matching, multi_stream_reader);
            reading_scheduler.run();
        }
        Mode::NaiveFiltered => {
            use crate::filtered_hyper_pattern_matching::FilteredHyperPatternMatching;
            let hyper_pattern_matching = FilteredHyperPatternMatching::<
                NaiveFilteredSingleHyperPatternMatching<ResultNotifierType>,
                ResultNotifierType,
            >::new(&automaton, result_notifier);
            let mut reading_scheduler =
                ReadingScheduler::new(hyper_pattern_matching, multi_stream_reader);
            reading_scheduler.run();
        }
        Mode::OnlineFiltered => {
            // use crate::filtered_hyper_pattern_matching::FilteredHyperPatternMatching;
            // let hyper_pattern_matching = FilteredHyperPatternMatching::<
            //     OnlineFilteredSingleHyperPatternMatching<ResultNotifierType>,
            //     ResultNotifierType,
            // >::new(&automaton, result_notifier);
            // let mut reading_scheduler =
            //     ReadingScheduler::new(hyper_pattern_matching, multi_stream_reader);
            // reading_scheduler.run();
        }
    }

    info!("Hyper Pattern Matching completed successfully");
}
