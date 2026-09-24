use divan::Bencher;
use glam::DVec3;
use muse_geom::icosphere;
use muse_ops::{
    MeshCalibration, PointwiseProgram, accumulate, advect, correlated_noise_calibrated, diffuse,
    gradient, noise as eval_noise, smooth_radius, voronoi_labels,
};
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

#[divan::bench]
fn voronoi_labels_level_5(bencher: Bencher) {
    let mesh = icosphere(5).unwrap();
    assert_eq!(mesh.positions.len(), 10_242);
    bencher.bench_local(|| voronoi_labels(&mesh, 9, "bench-voronoi", 12));
}

#[divan::bench]
fn advect_level_5(bencher: Bencher) {
    let mesh = icosphere(5).unwrap();
    assert_eq!(mesh.positions.len(), 10_242);
    let field: Vec<_> = (0..mesh.positions.len())
        .map(|i| (i as f64 * 0.001).sin())
        .collect();
    let velocity: Vec<_> = mesh
        .positions
        .iter()
        .map(|position| muse_geom::project_tangent(*position, DVec3::new(0.2, 0.3, 0.4)))
        .collect();
    bencher.bench_local(|| advect(&mesh, &field, &velocity));
}

#[divan::bench(args=[0.12, 0.20, 0.30, 0.62])]
fn smooth_radius_level_5(bencher: Bencher, radius: f64) {
    let mesh = icosphere(5).unwrap();
    let field: Vec<_> = (0..mesh.positions.len())
        .map(|i| (i as f64 * 0.13).sin())
        .collect();
    let calibration = MeshCalibration::new(&mesh).unwrap();
    eprintln!(
        "smooth_radius radius={radius:.2} level=5 cells={} neighborhood_pairs={}",
        field.len(),
        calibration.neighborhood_count(radius).unwrap()
    );
    bencher.bench_local(|| smooth_radius(&mesh, &field, radius));
}

#[divan::bench(args=[0.12, 0.20, 0.30, 0.62])]
fn smooth_radius_warm_calibration_level_5(bencher: Bencher, radius: f64) {
    let mesh = icosphere(5).unwrap();
    let calibration = MeshCalibration::new(&mesh).unwrap();
    let field: Vec<_> = (0..mesh.positions.len())
        .map(|i| (i as f64 * 0.13).sin())
        .collect();
    bencher.bench_local(|| calibration.smooth_radius(&field, radius));
}

#[divan::bench]
fn correlated_noise_three_scales_level_5(bencher: Bencher) {
    let mesh = icosphere(5).unwrap();
    let calibration = MeshCalibration::new(&mesh).unwrap();
    bencher.bench_local(|| {
        let _ = correlated_noise_calibrated(&calibration, &mesh, 9, "bench", 0.62, 1.0, 0.0);
        let _ = correlated_noise_calibrated(&calibration, &mesh, 9, "bench", 0.30, 1.0, 0.0);
        correlated_noise_calibrated(&calibration, &mesh, 9, "bench", 0.12, 1.0, 0.0)
    });
}

fn main() {
    divan::main();
}
