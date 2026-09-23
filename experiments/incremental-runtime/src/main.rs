use differential_dataflow::input::{Input, InputSession};
use rayon::prelude::*;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

const ITERATIONS: usize = 10;
const THRESHOLD: f64 = 0.5;
const TOLERANCE: f64 = 1.0e-10;

// Synthetic values remain finite; total_cmp gives Differential's Data an Ord.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct Number(f64);
impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0) == Ordering::Equal
    }
}
impl Eq for Number {}
impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Summary {
    mean: f64,
    above: usize,
}

fn summarize(values: impl Iterator<Item = f64>, count: usize) -> Summary {
    let mut sum = 0.0;
    let mut above = 0;
    for value in values {
        sum += value;
        above += usize::from(value > THRESHOLD);
    }
    Summary {
        mean: sum / count as f64,
        above,
    }
}

fn ring_step(values: &[f64]) -> Vec<f64> {
    let len = values.len();
    values
        .par_iter()
        .enumerate()
        .map(|(id, &center)| {
            (2.0 * center + values[(id + len - 1) % len] + values[(id + 1) % len]) / 4.0
        })
        .collect()
}

fn dense_field(input: &[f64]) -> Vec<f64> {
    let mut field: Vec<f64> = input.par_iter().map(|value| value.tanh()).collect();
    for _ in 0..ITERATIONS {
        field = ring_step(&field);
    }
    field
}

fn dense(input: &[f64]) -> Summary {
    summarize(dense_field(input).into_iter(), input.len())
}

fn input_values(size: usize) -> Vec<f64> {
    (0..size)
        .map(|id| ((id * 7919 % 10007) as f64) / 10007.0)
        .collect()
}

fn changed_input(original: &[f64], count: usize) -> Vec<f64> {
    let mut changed = original.to_vec();
    for (id, value) in changed.iter_mut().enumerate().take(count) {
        *value = (original[id] + 0.137).fract();
    }
    changed
}

fn current_summary(output: &BTreeMap<(usize, Number), isize>, size: usize) -> Summary {
    summarize(
        output.iter().flat_map(|((_, number), multiplicity)| {
            std::iter::repeat_n(number.0, (*multiplicity).max(0) as usize)
        }),
        size,
    )
}

#[derive(Debug)]
struct TimedResult {
    initial: Duration,
    one_percent: Duration,
    twenty_five_percent: Duration,
    initial_summary: Summary,
    one_percent_summary: Summary,
    twenty_five_percent_summary: Summary,
}

// One worker, one graph, and one source InputSession persist across all three epochs.
fn differential_run(original: &[f64]) -> TimedResult {
    let size = original.len();
    let original = original.to_vec();
    timely::execute_directly(move |worker| {
        let mut source = InputSession::<u64, (usize, Number), isize>::new();
        let output = Rc::new(RefCell::new(BTreeMap::<(usize, Number), isize>::new()));
        let output_capture = Rc::clone(&output);
        let probe = timely::dataflow::ProbeHandle::new();
        let (mut edge_input, _) = worker.dataflow::<u64, _, _>(|scope| {
            let edges = (0..size)
                .flat_map(|source| {
                    [
                        (source, (source, Number(2.0))),
                        (source, ((source + size - 1) % size, Number(1.0))),
                        (source, ((source + 1) % size, Number(1.0))),
                    ]
                })
                .collect::<Vec<_>>();
            let (edge_input, edge_collection) = scope.new_collection_from(edges);
            let field0 = source
                .to_collection(scope)
                .map(|(id, value)| (id, Number(value.0.tanh())));
            let mut field = field0;
            for _ in 0..ITERATIONS {
                let contributions = field.join_map(
                    edge_collection.clone(),
                    |_source, value, &(target, weight)| (target, Number(value.0 * weight.0)),
                );
                field = contributions.reduce(|_target, values, output| {
                    let weighted_sum: f64 = values
                        .iter()
                        .map(|(number, diff)| number.0 * *diff as f64)
                        .sum();
                    output.push((Number(weighted_sum / 4.0), 1));
                });
            }
            field
                .inspect(move |((id, number), _time, diff)| {
                    let mut current = output_capture.borrow_mut();
                    let key = (*id, *number);
                    let multiplicity = current.entry(key).or_insert(0);
                    *multiplicity += *diff;
                    if *multiplicity == 0 {
                        current.remove(&key);
                    }
                })
                .probe_with(&probe);
            (edge_input, ())
        });

        let drain = |worker: &mut timely::worker::Worker,
                     probe: &timely::dataflow::ProbeHandle<u64>,
                     source: &InputSession<u64, (usize, Number), isize>| {
            for step in 0..1_000_000usize {
                if !probe.less_than(source.time()) {
                    return;
                }
                worker.step();
                if step == 999_999 {
                    probe.with_frontier(|frontier| {
                        panic!(
                            "probe stalled at {frontier:?} waiting for input epoch {:?}",
                            source.time()
                        )
                    });
                }
            }
        };

        // Static edge input is committed at epoch zero and remains present thereafter.
        edge_input.advance_to(u64::MAX);
        edge_input.flush();
        let start = Instant::now();
        for (id, value) in original.iter().copied().enumerate() {
            source.update((id, Number(value)), 1);
        }
        source.advance_to(1);
        source.flush();
        drain(worker, &probe, &source);
        let initial_summary = current_summary(&output.borrow(), size);
        let initial = start.elapsed();

        let first_count = size / 100;
        let changed_one = changed_input(&original, first_count);
        let start = Instant::now();
        for id in 0..first_count {
            source.update((id, Number(original[id])), -1);
            source.update((id, Number(changed_one[id])), 1);
        }
        source.advance_to(2);
        source.flush();
        drain(worker, &probe, &source);
        let one_percent_summary = current_summary(&output.borrow(), size);
        let one_percent = start.elapsed();

        let final_count = size / 4;
        let changed_quarter = changed_input(&original, final_count);
        let start = Instant::now();
        for id in first_count..final_count {
            source.update((id, Number(original[id])), -1);
            source.update((id, Number(changed_quarter[id])), 1);
        }
        source.advance_to(3);
        source.flush();
        drain(worker, &probe, &source);
        let twenty_five_percent_summary = current_summary(&output.borrow(), size);
        let twenty_five_percent = start.elapsed();
        TimedResult {
            initial,
            one_percent,
            twenty_five_percent,
            initial_summary,
            one_percent_summary,
            twenty_five_percent_summary,
        }
    })
}

fn equivalent(left: Summary, right: Summary) {
    assert!(
        (left.mean - right.mean).abs() <= TOLERANCE,
        "means differ: {left:?} vs {right:?}"
    );
    assert_eq!(
        left.above, right.above,
        "threshold counts differ: {left:?} vs {right:?}"
    );
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

fn main() {
    for size in [10_000, 100_000] {
        let input = input_values(size);
        let _warmup = differential_run(&input);
        let mut dense_times = [Vec::new(), Vec::new(), Vec::new()];
        let mut differential_times = [Vec::new(), Vec::new(), Vec::new()];
        let mut summaries = [(Summary::default(), Summary::default()); 3];
        for _ in 0..5 {
            let measured = differential_run(&input);
            let differential_cases = [
                (0, measured.initial_summary, measured.initial),
                (
                    size / 100,
                    measured.one_percent_summary,
                    measured.one_percent,
                ),
                (
                    size / 4,
                    measured.twenty_five_percent_summary,
                    measured.twenty_five_percent,
                ),
            ];
            for (scenario, (changed, differential_summary, differential_elapsed)) in
                differential_cases.into_iter().enumerate()
            {
                let updated = changed_input(&input, changed);
                let start = Instant::now();
                let dense_summary = dense(&updated);
                let dense_elapsed = start.elapsed();
                equivalent(dense_summary, differential_summary);
                dense_times[scenario].push(dense_elapsed);
                differential_times[scenario].push(differential_elapsed);
                summaries[scenario] = (dense_summary, differential_summary);
            }
        }
        for (scenario, label) in ["initial", "1% update", "25% update"]
            .into_iter()
            .enumerate()
        {
            let changed = [0, size / 100, size / 4][scenario];
            println!("size={size} scenario={label} changed={changed} repetitions=5 dense_median={:?} differential_median={:?} dense={:?} differential={:?}", median(dense_times[scenario].clone()), median(differential_times[scenario].clone()), summaries[scenario].0, summaries[scenario].1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_total_order_is_consistent() {
        let numbers = [Number(-1.0), Number(0.0), Number(1.0)];
        assert!(numbers[0] < numbers[1] && numbers[1] < numbers[2]);
    }

    #[test]
    fn dense_scenarios_are_deterministic() {
        for size in [10_000, 100_000] {
            let original = input_values(size);
            for changed in [0, size / 100, size / 4] {
                let input = changed_input(&original, changed);
                assert_eq!(dense(&input), dense(&input));
            }
        }
    }

    #[test]
    fn tiny_persistent_frontier_advances_and_matches() {
        let input = input_values(100);
        let actual = differential_run(&input);
        equivalent(dense(&input), actual.initial_summary);
        equivalent(dense(&changed_input(&input, 1)), actual.one_percent_summary);
    }

    #[test]
    fn ten_thousand_persistent_frontier_and_updates_match() {
        let input = input_values(10_000);
        let actual = differential_run(&input);
        equivalent(dense(&input), actual.initial_summary);
        equivalent(
            dense(&changed_input(&input, 100)),
            actual.one_percent_summary,
        );
        equivalent(
            dense(&changed_input(&input, 2_500)),
            actual.twenty_five_percent_summary,
        );
    }

    #[test]
    fn hundred_thousand_persistent_frontier_and_updates_match() {
        let input = input_values(100_000);
        let actual = differential_run(&input);
        equivalent(dense(&input), actual.initial_summary);
        equivalent(
            dense(&changed_input(&input, 1_000)),
            actual.one_percent_summary,
        );
        equivalent(
            dense(&changed_input(&input, 25_000)),
            actual.twenty_five_percent_summary,
        );
    }

    #[test]
    fn persistent_differential_updates_match_dense() {
        for size in [10_000, 100_000] {
            let input = input_values(size);
            let result = differential_run(&input);
            for (count, actual) in [
                (0, result.initial_summary),
                (size / 100, result.one_percent_summary),
                (size / 4, result.twenty_five_percent_summary),
            ] {
                equivalent(dense(&changed_input(&input, count)), actual);
            }
        }
    }
}
