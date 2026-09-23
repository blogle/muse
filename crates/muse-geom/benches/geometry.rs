use divan::Bencher;
use muse_geom::{icosphere, project_tangent};

fn main() {
    divan::main();
}

#[divan::bench]
fn level_five_construction(bencher: Bencher) {
    bencher.bench(|| icosphere(5).unwrap());
}

#[divan::bench]
fn level_five_tangent_projection(bencher: Bencher) {
    let mesh = icosphere(5).unwrap();
    bencher.bench(|| {
        mesh.positions
            .iter()
            .map(|position| project_tangent(*position, glam::DVec3::ONE))
            .collect::<Vec<_>>()
    });
}
