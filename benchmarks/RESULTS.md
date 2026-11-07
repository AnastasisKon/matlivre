# Benchmark: MATLAB vs Octave vs Scilab vs MatLivre

`bench.m` (with `bench_fib.m`) runs unchanged in MATLAB, Octave and MatLivre. Scilab runs
`bench.sce`, a line-by-line port with the same cases and sizes (Scilab's syntax differs). Each
case reports the best of 3 runs (`tic`/`toc`), and a checksum so the engines can be checked
against each other: **all 12 checksums agree across MATLAB, Octave, Scilab and MatLivre.**

- Machine: Intel Core i5-10600K (6 cores / 12 threads), 24 GB RAM, Windows 11 Pro.
- MATLAB R2023b Update 3 (`matlab -batch`), GNU Octave 11.3.0 (`octave-cli`), Scilab 2025.0.0
  (`WScilex-cli -nb`), matlivre release build (`matlivre-cli`). Runs were sequential, not
  concurrent, on an idle machine after a restart.
- Run date: 2026-11-07. The MatLivre column was re-measured after switching to mimalloc and
  reusing the LU buffer (see below); the other engines are from the first run the same day.

| Case | MATLAB (ms) | Octave (ms) | Scilab (ms) | MatLivre (ms) | vs MATLAB | vs Octave | vs Scilab |
|---|---:|---:|---:|---:|---|---|---|
| scalar_loop (2M iterations) | 4.57 | 2,267.82 | 1,662.67 | 2.71 | 1.7x faster | 836x faster | 613x faster |
| while_branch (Collatz, 2.9M steps) | 79.62 | 11,231.69 | 70,510.38 | 14.54 | 5.5x faster | 773x faster | 4,850x faster |
| fib_recursive (fib(20)) | 0.96 | 123.42 | 70.68 | 0.06 | 16.1x faster | 2,057x faster | 1,178x faster |
| array_growth (100k appends) | 6.83 | 377.48 | 211.68 | 1.20 | 5.7x faster | 316x faster | 177x faster |
| indexed_assign (1M writes) | 2.93 | 1,996.90 | 879.51 | 2.45 | 1.2x faster | 814x faster | 359x faster |
| vector_math (2M elements) | 107.40 | 92.81 | 180.59 | 49.36 | 2.2x faster | 1.9x faster | 3.7x faster |
| matmul_300 | 0.68 | 0.55 | 0.19 | 0.32 | 2.2x faster | 1.8x faster | 1.7x slower |
| linsolve_300 | 0.53 | 1.54 | 0.48 | 0.59 | 1.1x slower | 2.6x faster | 1.2x slower |
| sort_1e6 | 59.09 | 14.82 | 185.38 | 8.91 | 6.6x faster | 1.7x faster | 20.8x faster |
| string_build (20k appends) | 207.04 | 249.86 | 491.80 | 35.61 | 5.8x faster | 7.0x faster | 13.8x faster |
| struct_fields (500k updates) | 144.11 | 4,309.09 | 1,841.38 | 0.69 | 210x faster | 6,272x faster | 2,680x faster |
| cell_ops (100k writes + reads) | 21.54 | 561.40 | 349.36 | 7.55 | 2.9x faster | 74.4x faster | 46.3x faster |

Geometric mean over the 12 cases: MatLivre is **4.6x faster than MATLAB, 71x faster than Octave
and 70x faster than Scilab**. MatLivre is faster than MATLAB in 11 of 12 cases (behind in
`linsolve_300`), than Octave in all 12 and than Scilab in 10 (behind in `matmul_300` and
`linsolve_300`; Scilab ships Intel oneMKL).

Caveats: cases under 1 ms vary by tens of percent between runs (MATLAB's `matmul_300` measured
1.81 ms on 2026-10-29 and 0.69 ms here). The suite is small and favours native loop compilation;
it does not measure toolbox breadth, plotting or numerical accuracy beyond the checksums.

## What made the difference

- **Native compilation of loops and functions** (`src/jit.rs`, Cranelift). `for`/`while` loops
  over scalar doubles and logicals, elements of numeric arrays (`x(k)`, `A(i,j)`, `x(end+1)`),
  scalar cell elements (`c{k}`) and scalar struct fields (`p.x`) compile to machine code, as do
  user functions called with scalar doubles, including recursion. Anything else (display,
  strings, possibly complex results such as `sqrt` of a negative, whole-array use) stays
  interpreted, so behaviour is unchanged. Differential tests (`tests/jit.rs`) run every snippet
  with compilation on and off and require identical output, errors and workspace.
  `MATLIVRE_NOJIT=1` turns compilation off.
- **Linear algebra through `faer`**: blocked, multithreaded matrix products (one thread per
  physical core) and a blocked LU for `\`, `inv` and `det` from order 24.
- **Parallel elementwise math** (rayon) from 32k elements, and `x.^2` as `x.*x`.
- **Parallel unstable sort** when only sorted values are requested (sort stays stable when
  `-0` and `0` both occur or indices are requested).
- **In-place append** for `s = [s expr]` instead of copying the whole array each time.
- **Amortized `x(end+1) = v`**, in-place cell element writes, and operands of binary operators
  read by reference instead of copied.
- **mimalloc as the global allocator.** With the system allocator, every large array came back
  from Windows as untouched pages, and faulting them in cost about a third of `matmul_300` and
  up to half of a whole-array operation. `matmul_300` went from ~0.5 to ~0.34 ms.
- **LU working buffer kept between solves**: `linsolve_300` went from 0.78 to 0.60 ms.

## Reproduce

```bash
run_bench.bat
```

It runs the four engines one after another and prints this table (`compare.ps1`).
