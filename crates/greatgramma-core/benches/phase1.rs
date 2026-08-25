use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[cfg(not(target_os = "linux"))]
use std::process::Command;

use greatgramma_core::{
    Action, AdvanceResult, DfaStateId, LalrDimensions, LalrTable, LexerDfa, NonterminalId,
    ParserStateId, PreparationLimits, Production, ProductionId, TerminalId, TokenEntry, TokenId,
    UnvalidatedGrammar, ValidatedGrammar, ValidationLimits, prepare,
};

const SCHEMA: &str = "greatgramma.phase1.benchmark.v1";
const ITEM: TerminalId = TerminalId::new(0);
const EOF: TerminalId = TerminalId::new(1);
const LIST: NonterminalId = NonterminalId::new(0);

fn main() -> ExitCode {
    let mut leak_check = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--bench" => {}
            "--leak-check" => leak_check = true,
            _ => {
                eprintln!("unknown benchmark argument: {argument}");
                return ExitCode::from(2);
            }
        }
    }

    if leak_check {
        match run_leak_check() {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(error) => {
                eprintln!("leak check unavailable: {error}");
                ExitCode::from(2)
            }
        }
    } else {
        run_benchmarks();
        ExitCode::SUCCESS
    }
}

fn run_benchmarks() {
    for (label, vocab_size, prepare_iterations, mask_iterations, advance_iterations) in [
        ("small", 256, 20, 1_000, 50_000),
        ("medium", 4_096, 4, 200, 50_000),
        ("vocab-32k", 32_000, 3, 100, 20_000),
        ("vocab-50k", 50_000, 2, 60, 20_000),
        ("vocab-128k", 128_000, 1, 30, 10_000),
    ] {
        benchmark_prepare(label, vocab_size, prepare_iterations);
        benchmark_mask(label, vocab_size, mask_iterations);
        benchmark_advance(label, vocab_size, advance_iterations);
    }
}

fn benchmark_prepare(label: &str, vocab_size: usize, iterations: u64) {
    drop(prepare_grammar(validated_grammar(vocab_size)));

    let mut elapsed = Duration::ZERO;
    let mut samples = Vec::with_capacity(usize::try_from(iterations).expect("sample count"));
    for _ in 0..iterations {
        let grammar = validated_grammar(vocab_size);
        let started = Instant::now();
        let prepared = prepare_grammar(grammar);
        let duration = started.elapsed();
        elapsed += duration;
        samples.push(duration.as_nanos());
        black_box(prepared.token_count());
    }
    emit_timing(
        label, "prepare", vocab_size, iterations, elapsed, samples, 0,
    );
}

fn benchmark_mask(label: &str, vocab_size: usize, iterations: u64) {
    let mut matcher = prepare_grammar(validated_grammar(vocab_size))
        .into_matcher(1)
        .expect("one benchmark row");
    assert_eq!(
        matcher.advance(0, TokenId::new(0)),
        Ok(AdvanceResult::Continue)
    );
    let mut mask = vec![0_u8; matcher.mask_bytes()];
    matcher.mask(0, &mut mask).expect("live benchmark mask");

    let mut checksum = 0_u64;
    let mut elapsed = Duration::ZERO;
    let mut samples = Vec::with_capacity(usize::try_from(iterations).expect("sample count"));
    for iteration in 0..iterations {
        let started = Instant::now();
        matcher.mask(0, &mut mask).expect("live benchmark mask");
        let duration = started.elapsed();
        elapsed += duration;
        samples.push(duration.as_nanos());
        let index = usize::try_from(iteration).expect("benchmark iteration") % mask.len();
        checksum = checksum.wrapping_add(u64::from(mask[index]));
        black_box(&mask);
    }
    emit_timing(
        label, "mask", vocab_size, iterations, elapsed, samples, checksum,
    );
}

fn benchmark_advance(label: &str, vocab_size: usize, iterations: u64) {
    let mut matcher = prepare_grammar(validated_grammar(vocab_size))
        .into_matcher(1)
        .expect("one benchmark row");
    let ordinary_tokens = vocab_size - 1;

    for iteration in 0..100_u64 {
        let token = token_for_iteration(iteration, ordinary_tokens);
        black_box(
            matcher
                .advance(0, token)
                .expect("recursive benchmark token"),
        );
    }

    let mut elapsed = Duration::ZERO;
    let mut samples = Vec::with_capacity(usize::try_from(iterations).expect("sample count"));
    for iteration in 0..iterations {
        let token = token_for_iteration(iteration, ordinary_tokens);
        let started = Instant::now();
        black_box(
            matcher
                .advance(0, token)
                .expect("recursive benchmark token"),
        );
        let duration = started.elapsed();
        elapsed += duration;
        samples.push(duration.as_nanos());
    }
    emit_timing(
        label, "advance", vocab_size, iterations, elapsed, samples, 0,
    );
}

fn emit_timing(
    size: &str,
    operation: &str,
    vocab_size: usize,
    iterations: u64,
    elapsed: Duration,
    mut samples: Vec<u128>,
    checksum: u64,
) {
    let elapsed_ns = elapsed.as_nanos();
    let ns_per_iteration = elapsed_ns / u128::from(iterations);
    samples.sort_unstable();
    let p50_ns = percentile(&samples, 50);
    let p95_ns = percentile(&samples, 95);
    let p99_ns = percentile(&samples, 99);
    println!(
        "{{\"schema\":\"{SCHEMA}\",\"kind\":\"timing\",\"size\":\"{size}\",\"operation\":\"{operation}\",\"vocab_size\":{vocab_size},\"iterations\":{iterations},\"elapsed_ns\":{elapsed_ns},\"ns_per_iteration\":{ns_per_iteration},\"p50_ns\":{p50_ns},\"p95_ns\":{p95_ns},\"p99_ns\":{p99_ns},\"checksum\":{checksum}}}"
    );
}

fn percentile(samples: &[u128], percentage: usize) -> u128 {
    let rank = samples.len().saturating_mul(percentage).saturating_add(99) / 100;
    samples[rank.saturating_sub(1).min(samples.len() - 1)]
}

fn run_leak_check() -> Result<bool, String> {
    const STEPS: u64 = 100_000;
    const SESSION_STEPS: u64 = 1_000;
    const SAMPLE_INTERVAL: u64 = 10_000;
    const MAX_GROWTH_KIB: u64 = 1_024;
    const MAX_SLOPE_BYTES_PER_STEP: f64 = 8.0;

    let vocab_size = 256;
    let ordinary_tokens = vocab_size - 1;
    black_box(current_rss_kib()?);

    let mut samples = Vec::with_capacity(usize::try_from(STEPS / SAMPLE_INTERVAL).unwrap());
    let session_count = STEPS / SESSION_STEPS;
    for session in 0..session_count {
        let mut matcher = prepare_grammar(validated_grammar(vocab_size))
            .into_matcher(1)
            .expect("one leak-check row");
        let mut mask = vec![0_u8; matcher.mask_bytes()];
        for offset in 1..=SESSION_STEPS {
            let step = session * SESSION_STEPS + offset;
            let token = token_for_iteration(step, ordinary_tokens);
            black_box(
                matcher
                    .advance(0, token)
                    .expect("recursive leak-check token"),
            );
            matcher.mask(0, &mut mask).expect("live leak-check mask");
            black_box(&mask);
        }
        let step = (session + 1) * SESSION_STEPS;
        if step.is_multiple_of(SAMPLE_INTERVAL) {
            let rss_kib = current_rss_kib()?;
            println!(
                "{{\"schema\":\"{SCHEMA}\",\"kind\":\"rss_sample\",\"step\":{step},\"rss_kib\":{rss_kib}}}"
            );
            samples.push((step, rss_kib));
        }
    }

    let first = samples.first().copied().ok_or("missing first RSS sample")?;
    let last = samples.last().copied().ok_or("missing last RSS sample")?;
    let growth_kib = last.1.saturating_sub(first.1);
    let slope = rss_slope_bytes_per_step(&samples);
    let passed = growth_kib <= MAX_GROWTH_KIB && slope <= MAX_SLOPE_BYTES_PER_STEP;
    println!(
        "{{\"schema\":\"{SCHEMA}\",\"kind\":\"rss_result\",\"steps\":{STEPS},\"first_rss_kib\":{},\"last_rss_kib\":{},\"growth_kib\":{growth_kib},\"max_growth_kib\":{MAX_GROWTH_KIB},\"slope_bytes_per_step\":{slope:.6},\"max_slope_bytes_per_step\":{MAX_SLOPE_BYTES_PER_STEP:.6},\"passed\":{passed}}}",
        first.1, last.1
    );
    Ok(passed)
}

fn rss_slope_bytes_per_step(samples: &[(u64, u64)]) -> f64 {
    let count = samples.len() as f64;
    let mean_step = samples.iter().map(|sample| sample.0 as f64).sum::<f64>() / count;
    let mean_rss = samples.iter().map(|sample| sample.1 as f64).sum::<f64>() / count;
    let covariance = samples
        .iter()
        .map(|sample| (sample.0 as f64 - mean_step) * (sample.1 as f64 - mean_rss))
        .sum::<f64>();
    let variance = samples
        .iter()
        .map(|sample| (sample.0 as f64 - mean_step).powi(2))
        .sum::<f64>();
    (covariance / variance) * 1_024.0
}

fn current_rss_kib() -> Result<u64, String> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status")
            .map_err(|error| format!("read /proc/self/status: {error}"))?;
        let line = status
            .lines()
            .find(|line| line.starts_with("VmRSS:"))
            .ok_or("VmRSS is absent from /proc/self/status")?;
        line.split_whitespace()
            .nth(1)
            .ok_or_else(|| "VmRSS has no numeric value".to_owned())?
            .parse()
            .map_err(|error| format!("parse VmRSS: {error}"))
    }

    #[cfg(not(target_os = "linux"))]
    {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .map_err(|error| format!("run ps: {error}"))?;
        if !output.status.success() {
            return Err(format!("ps exited with {}", output.status));
        }
        std::str::from_utf8(&output.stdout)
            .map_err(|error| format!("decode ps output: {error}"))?
            .trim()
            .parse()
            .map_err(|error| format!("parse ps RSS: {error}"))
    }
}

fn token_for_iteration(iteration: u64, ordinary_tokens: usize) -> TokenId {
    let token = usize::try_from(iteration).expect("benchmark iteration") % ordinary_tokens;
    TokenId::new(u32::try_from(token).expect("benchmark token ID"))
}

fn prepare_grammar(grammar: ValidatedGrammar) -> greatgramma_core::PreparedGrammar {
    prepare(grammar, PreparationLimits::default()).expect("benchmark grammar prepares")
}

fn validated_grammar(vocab_size: usize) -> ValidatedGrammar {
    assert!(vocab_size >= 2);
    grammar(vocab_size)
        .validate(ValidationLimits::default())
        .expect("benchmark grammar validates")
}

fn grammar(vocab_size: usize) -> UnvalidatedGrammar {
    let mut byte_classes = vec![1_u32; 256];
    byte_classes[usize::from(b'a')] = 0;
    let lexer = LexerDfa::new(
        2,
        2,
        byte_classes,
        vec![Some(DfaStateId::new(1)), None, None, None],
        DfaStateId::new(0),
        vec![None, Some(ITEM)],
    );

    let reduce = |production: u32| Action::Reduce {
        production: ProductionId::new(production),
        rank: 1,
    };
    let lalr = LalrTable::new(
        LalrDimensions::new(4, 2, 1),
        ParserStateId::new(0),
        EOF,
        vec![
            Action::Shift(ParserStateId::new(1)),
            Action::Error,
            reduce(0),
            reduce(0),
            Action::Shift(ParserStateId::new(3)),
            Action::Accept,
            reduce(1),
            reduce(1),
        ],
        vec![Some(ParserStateId::new(2)), None, None, None],
        vec![Production::new(LIST, 1), Production::new(LIST, 2)],
    );

    let mut tokens = Vec::with_capacity(vocab_size);
    for token in 0..(vocab_size - 1) {
        tokens.push(TokenEntry::Bytes(vec![b'a'; token % 8 + 1]));
    }
    tokens.push(TokenEntry::Eos);
    UnvalidatedGrammar::new(tokens, lexer, lalr)
}
