use divan::Bencher;
use glam::DVec3;
use muse_ops::{PointwiseProgram, accumulate, diffuse, gradient, noise as eval_noise};
use muse_types::Mesh;
use std::collections::BTreeMap;

fn mesh(n: usize) -> Mesh {
    Mesh {
        positions: (0..n)
            .map(|i| {
                let x = i as f64;
                DVec3::new(x, (x * 0.01).sin(), 0.0)
            })
            .collect(),
        triangles: vec![],
        neighbors: (0..n)
            .map(|i| {
                [i.checked_sub(1), (i + 1 < n).then_some(i + 1)]
                    .into_iter()
                    .flatten()
                    .map(|j| j as u32)
                    .collect()
            })
            .collect(),
    }
}

#[divan::bench(args=[10_000,100_000])]
fn noise(bencher: Bencher, n: usize) {
    let m = mesh(n);
    bencher.bench_local(|| eval_noise(&m, 9, "bench", 1.0));
}
#[divan::bench(args=[10_000,100_000])]
fn pointwise(bencher: Bencher, n: usize) {
    let m = mesh(n);
    let mut bindings = BTreeMap::new();
    bindings.insert("x".to_owned(), vec![2.0; n]);
    let program = PointwiseProgram::compile("x * 2.0 + 1.0").unwrap();
    bencher.bench_local(|| program.execute(&m, &bindings, &BTreeMap::new()));
}
#[divan::bench]
fn gradient_10k(bencher: Bencher) {
    let m = mesh(10_000);
    let v = vec![1.0; 10_000];
    bencher.bench_local(|| gradient(&m, &v));
}
#[divan::bench]
fn diffuse_10k(bencher: Bencher) {
    let m = mesh(10_000);
    let v = vec![1.0; 10_000];
    bencher.bench_local(|| diffuse(&m, &v, 0.2, 1));
}
#[divan::bench]
fn accumulate_10k(bencher: Bencher) {
    let receivers = (0..10_000)
        .map(|i| if i == 9_999 { i as u32 } else { (i + 1) as u32 })
        .collect::<Vec<_>>();
    let values = vec![1.0; 10_000];
    bencher.bench_local(|| accumulate(&receivers, &values));
}

fn main() {
    divan::main();
}
