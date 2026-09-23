# Incremental runtime dependency blocker

An experiment-local manifest and lockfile are deferred because a compatible, buildable Timely/Differential pair could not be established safely during coordinator preflight.

Observed candidate: `differential-dataflow = 0.15.0` requires `timely = 0.21` and `columnar = 0.4`. Pinning the directly declared `timely = 0.19.0` alongside it brought in incompatible Timely/Columnar generations, producing compile errors in `timely_communication`. Aligning the direct Timely version to `0.21.5` still produced derive/API mismatch errors in `differential-dataflow 0.15.0` against resolved `columnar 0.4.1`. No transitive overrides or extra dependencies are authorized by this task.

W1-F should confirm a mutually compatible Timely/Differential release pair and its transitive lock graph before adding experiment-local manifest entries. Neither dependency has been added to root `[workspace.dependencies]`.
