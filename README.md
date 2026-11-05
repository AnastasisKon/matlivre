# MatLivre

<!-- release-manager:download -->
[![Download MatLivre 0.1.2](https://img.shields.io/badge/Download-v0.1.2-005FB8?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/campuslivre/matlivre/releases/download/v0.1.2/matlivre-win-Setup.exe)

[matlivre-win-Setup.exe](https://github.com/campuslivre/matlivre/releases/download/v0.1.2/matlivre-win-Setup.exe) · Windows installer, version 0.1.2
<!-- /release-manager:download -->

**MatLivre is a fast live script editor and runtime for MATLAB code.** It opens, edits and runs
MATLAB live scripts (`.mlx`) and plain scripts (`.m`) with results, tables and figures inline,
in a native Windows 11 app. The language engine is an independent, clean-room implementation
of MATLAB, written in Rust: no MATLAB installation, license or Java runtime is needed.

![A live script running in MatLivre](docs/welcome.png)

- **Starts in a blink.** The app is ready in 0.3 s (1 s on a first launch). MATLAB needs
  about 4 s for its desktop to run code and 24 s on a cold first start.
- **Runs live scripts fast.** A typical notebook with four figures runs in 0.11 s. MATLAB's
  Live Editor takes 4.9 s for the same file, and 14.3 s the first time: 43 to 100 times slower.
- **Runs code fast.** On a 12-case benchmark of loops, recursion, array growth, linear
  algebra, sorting, strings, structs and cells, MatLivre is 4.7x faster than MATLAB R2023b and
  65x faster than GNU Octave (geometric mean), with identical results in all three.
- **Speaks MATLAB.** 1,830 MATLAB and Symbolic Math Toolbox functions, MATLAB's display format,
  error messages and semantics, checked against MATLAB R2023b output.
- **One small program.** A single 36 MB executable that depends only on DLLs shipped with
  Windows.

## Performance

All measurements on the same machine: Intel Core i5-10600K (6 cores / 12 threads), 24 GB RAM,
Windows 11 Pro. MATLAB R2023b Update 3, GNU Octave 11.3.0, MatLivre release build. Programs
were run one after another, never concurrently.

### Cold start

Time from launching the program until it has run a line of code. For the desktop apps this is
the time until you can work in them.

| Program | First launch | Warm launch |
|---|---:|---:|
| **MatLivre app** (window up, ready to run) | **0.97 s** | **0.31 s** |
| **MatLivre CLI** (runs a script) | **0.03 s** | **0.07 s** |
| GNU Octave CLI (`--eval`) | 2.56 s | 0.35 s |
| MATLAB desktop (runs a startup command) | — | 3.90 s |
| MATLAB `-batch` | 24.30 s | 6.84 s |

"First launch" is the first start in a session (MATLAB's was a true cold start: 24 s before
it ran anything); "warm launch" is the median of three more starts, with the program's files
already in the disk cache. MATLAB's desktop was measured after its batch runs, so only its warm
time is shown. The Octave GUI is not in the table: its first start stops at a setup wizard.
Script: [`benchmarks/coldstart.ps1`](benchmarks/coldstart.ps1).

### Live scripts

[`benchmarks/livescript/bench_live.mlx`](benchmarks/livescript/bench_live.mlx) is a typical
analysis notebook: eight sections with matrix algebra, a loop, statistics, a table, string
processing and four figures (a line plot, a surface with a colorbar, a histogram and a 2x2
grid of subplots). The time covers running every section, capturing the outputs and drawing
the figures.

| Engine | First run | Warm run |
|---|---:|---:|
| **MatLivre** | **0.14 s** | **0.11 s** |
| MATLAB Live Editor (`executeAndSave`) | 14.30 s | 4.91 s |
| MATLAB, same code as a plain `.m` script | 1.17 s | 0.37 s |
| GNU Octave | cannot run live scripts | |

MatLivre is about **43x faster** than MATLAB's Live Editor on a warm run and **100x faster** on
the first run in a session. MATLAB runs the same code as a plain script in 0.37 s, so most of
its live script time is the Live Editor itself: evaluating section by section, capturing each
output and figure, and writing them into the file. MatLivre's time covers loading the `.mlx`,
running every section, capturing the outputs and rendering the four figures (it does not write
outputs back into the file). Warm runs are the best of three in an already running program.
Scripts: [`benchmarks/livescript`](benchmarks/livescript).

**Does Octave run live scripts?** No. GNU Octave has no live script support: it cannot open or
run `.mlx` files, and it has no live editor with inline output. The closest it offers is `%%`
code sections in plain `.m` files in its editor, with output going to the command window and
figures to separate windows.

### Language and numerics

[`benchmarks/bench.m`](benchmarks/bench.m) runs unchanged in all three engines; each case is
the best of 3 runs and reports a checksum, and **all 12 checksums agree**.

| Case | MATLAB (ms) | Octave (ms) | MatLivre (ms) | MatLivre vs MATLAB |
|---|---:|---:|---:|---|
| scalar_loop (2M iterations) | 4.92 | 2,265.87 | 2.62 | 1.9x faster |
| while_branch (Collatz, 2.9M steps) | 102.22 | 11,742.56 | 13.86 | 7.4x faster |
| fib_recursive (fib(20)) | 0.66 | 128.88 | 0.06 | 11.3x faster |
| array_growth (100k appends) | 8.43 | 402.17 | 1.42 | 5.9x faster |
| indexed_assign (1M writes) | 4.01 | 2,115.03 | 3.15 | 1.3x faster |
| vector_math (2M elements) | 121.78 | 127.25 | 86.36 | 1.4x faster |
| matmul_300 | 1.81 | 0.51 | 0.41 | 4.5x faster |
| linsolve_300 | 0.61 | 1.85 | 0.73 | 1.2x slower |
| sort_1e6 | 58.93 | 17.77 | 9.36 | 6.3x faster |
| string_build (20k appends) | 201.63 | 264.22 | 29.53 | 6.8x faster |
| struct_fields (500k updates) | 170.42 | 4,568.02 | 0.68 | 252x faster |
| cell_ops (100k writes + reads) | 31.25 | 581.45 | 17.70 | 1.8x faster |

Geometric mean: **4.7x faster than MATLAB, 64.8x faster than Octave**. Details and the
per-case Octave comparison are in [`benchmarks/RESULTS.md`](benchmarks/RESULTS.md).

### Why it is fast

- **Native code for loops and functions.** `for`/`while` loops and user functions over scalars,
  array elements, cell elements and struct fields are compiled to machine code with Cranelift.
  Everything else is interpreted, with identical behavior (differential tests run every case
  both ways).
- **Multithreaded linear algebra** through `faer`: blocked matrix products and LU
  factorization.
- **Parallel elementwise math and sorting** on large arrays (rayon).
- **No copies where MATLAB semantics allow it**: in-place appends (`x(end+1) = v`,
  `s = [s v]`), in-place element writes, operands read by reference.
- **A small runtime**: no JVM, no desktop framework to boot; the app is one native process.

## The app

![Figures next to the code that draws them](docs/graphics.png)

- Live scripts with formatted text, code, and results inline or on the right; run a section
  (Ctrl+Enter), run and advance (Ctrl+Shift+Enter) or run all (F5), and stop between
  statements.
- Live controls (sliders, drop-downs, check boxes, edit fields) in `.mlx` files.
- Figures drawn inline: 2-D and 3-D plots, surfaces with lighting, contours, images, bar, pie,
  histogram, polar plots and more; charts (heatmaps, box, swarm and bubble charts, pie and
  donut charts, word clouds, stacked and parallel coordinates plots); two y axes (`yyaxis`),
  secondary axis labels, date ticks, tiled layouts, annotations and colorbars, with a crosshair that reads values off
  the plot. `saveas`, `print` and `exportgraphics` write PNG and SVG.
- A files panel (open a folder as the project root, like VS Code), workspace browser, command window and floating figure windows; tabs that can
  be dragged between windows; light and dark themes.
- Code runs on its own engine thread, so the editor never freezes while a script runs.
- Files changed by another program (an editor, Claude Code, git) reload in their tab, as in
  VS Code and MATLAB. A tab with unsaved changes asks instead: reload the file or keep your
  version, and it never saves over a change it has not shown you.
- Claude, through the Claude Code installed on your computer (no API key):
  - **Claude panel** (Ctrl+L): chat with Claude, which can run code in your workspace, see
    your figures, and read and edit the open documents. It asks before it runs or changes
    anything.
  - **Code completions**: after a pause in typing, Claude suggests the rest of the line or
    block in grey; Tab inserts it, Esc dismisses it, Alt+\ asks at once. Toggle them in the
    … menu.
  - **MCP server**: … › Connect Claude Code copies a `claude mcp add` command, after which
    Claude Code in a terminal works in the open MatLivre window (or in a MatLivre without a
    window, `matlivre --mcp`, when none is open).

| Shortcut | Action |
|---|---|
| Ctrl+Enter | Run current section |
| Ctrl+Shift+Enter | Run section and advance |
| F5 | Run all |
| Ctrl+Alt+Enter | Insert section break |
| Ctrl+N / Ctrl+O / Ctrl+S | New / Open / Save |
| Ctrl+L | Show or hide the Claude panel |
| Tab / Esc / Alt+\ | Accept / dismiss / request a code suggestion |

## The language

MATLAB compatibility is the rule: semantics, display and error messages follow MATLAB R2023b.
[`docs/matlab-coverage.md`](docs/matlab-coverage.md) lists every function of the MATLAB and
Symbolic Math Toolbox references and its status (1,830 implemented).

- N-D arrays of every numeric type, `logical`, `char` and strings, with implicit expansion,
  saturating integer arithmetic and complex numbers; regular expressions and text patterns
  (`digitsPattern + "-" + lettersPattern`).
- Cell arrays, struct arrays, function handles, anonymous and `inline` functions, `memoize`, `containers.Map`,
  `dictionary`, `categorical`, `table` (joins, grouping, reshaping), `timetable`
  (`retime`, `synchronize`, `timerange`), `timeseries` and `tscollection`, `datetime` (with IANA time
  zones and daylight saving time), `duration` and `calendarDuration`.
- `classdef` classes (properties, methods, static methods, inheritance, handle and value
  semantics, operator overloading, events and listeners, enumerations, System objects with
  `step`, `setup` and `release`) and function files from the script's folder; argument
  validation (`mustBe*`, `inputParser`), `Name=Value` arguments and error identifiers;
  `timer` objects, `diary`, `localfunctions` and the `settings` API.
- Graphs and networks: `graph` and `digraph` with shortest paths, search, components,
  spanning trees, topological sort, maximum flow, centrality and plotting.
- Sparse matrices: `sparse`, `spdiags`, `speye`, iterative solvers (`pcg`, `gmres`,
  `bicgstab`, `minres`, `lsqr`, ...), `ichol`, `ilu`, reorderings and `spy`.
- Mathematics: linear algebra and factorizations (with `decomposition` objects), polynomials and interpolation, ODE solvers
  (nonstiff, stiff, DAEs with mass matrices, fully implicit, output functions, also through `ode` objects), delay and boundary value problems,
  parabolic PDEs (`pdepe`), integration, minimization and root finding, statistics, FFT and filtering, computational geometry
  (polyshape booleans and buffers, triangulations, STL files), special
  functions, image resizing and indexed color (`imresize`, `rgb2ind`, `cmunique`).
- Symbolic Math: exact arithmetic, `diff`, `int`, `limit`, `taylor`, `series`, `solve`,
  `vpasolve`, Laplace, Z, Fourier and Hilbert transforms, `simplify`, `partfrac`, symbolic
  matrices with Jordan, Smith and Hermite forms, Groebner bases, `piecewise`, abstract
  functions (`syms y(t)`) with `dsolve` for linear differential equations, `combine`,
  `root`, `eliminate`, MathML output, `sympref`, number theory,
  special functions (elliptic integrals and Jacobi elliptic functions, exponential,
  trigonometric and Fresnel integrals, polylogarithms, hypergeometric functions), the
  classical orthogonal polynomials, polynomial division and reduction, resultants, Pade
  approximants, scalar and vector potentials, numeric sums and integrals (`vpasum`,
  `vpaintegral`), the symType queries (`symType`, `findSymType`, `mapSymType`) and units of
  measurement (`symunit`, `unitConvert`, `checkUnits`, `newUnit`).
- Files: text and CSV I/O, Excel workbooks (`readtable`, `xlsread`, `writetable`), MAT files, images (with `imtile` and the `imformats` registry) and audio, the `Tiff` class (tags, strips, tiles, LZW and Deflate), JSON and XML,
  zip and tar archives.

Try [`examples/welcome.m`](examples/welcome.m), [`examples/graphics.m`](examples/graphics.m) and
[`examples/features.m`](examples/features.m).

## Build from source

```bash
cargo run -p livre-app --release
```

Pass a file to open it: `cargo run -p livre-app --release -- path\to\script.mlx`. On Windows,
`build.bat` builds and `run.bat [file]` builds and starts the app. `target\release\matlivre.exe`
links the C runtime statically (`.cargo/config.toml`), so it runs on any Windows 10/11 machine
without installing anything.

`matlivre-cli script.m` runs a script headless and prints its output.

## Project layout

| Crate | Purpose |
|---|---|
| `crates/livre-lang` | Lexer, parser, interpreter and native-code compiler, values and MATLAB display, builtins, classdef, Symbolic Math, figure model |
| `crates/livre-doc` | Live script document model; `.mlx` (OPC/ZIP + WordprocessingML) and `.m` reader/writer |
| `crates/livre-app` | The desktop app (eframe/egui): editor, outputs, plots, workspace, command window |

## Clean-room process

MatLivre was written from observed behavior only. The live script format is described in
[`docs/SPEC-live-script.md`](docs/SPEC-live-script.md), produced by inspecting the `.mlx` data
files shipped with MATLAB and the public behavior of the Live Editor. Language behavior is
checked by running code in MATLAB and comparing outputs. No MathWorks source code was read or
copied.

## Tests

```bash
cargo test --workspace
```

The tests compare MatLivre's output with MATLAB R2023b's for each function family.
`livre-doc/tests/matlab_samples.rs` round-trips the live scripts shipped with a local MATLAB
installation when one is present. Benchmarks: [`benchmarks/run_bench.bat`](benchmarks/run_bench.bat)
(language), [`benchmarks/coldstart.ps1`](benchmarks/coldstart.ps1) (start-up) and
[`benchmarks/livescript`](benchmarks/livescript) (live scripts).

## Known limits

- Saved outputs in `.mlx` files are not restored when opening (run the script to regenerate
  them) and are not written back.
- Rich text in live scripts (bold and italic runs, links, equations, images) is shown as plain
  text.
- Functions still to implement are listed in [`docs/matlab-coverage.md`](docs/matlab-coverage.md),
  among them the differential-algebraic equation tools, symbolic code generation (`ccode`,
  `fortran`), `subexpr`, and spreadsheet and XML import options.
- The Symbolic Math special functions evaluate to double precision only: `vpa` keeps
  `sinint(2)` and the like in symbolic form (`catalan` and `eulergamma` evaluate). Some
  complex-valued cases (`logint` and `igamma` of negative numbers, `ellipticPi` with `n > 1`)
  give `NaN`, and polynomials with symbolic parameters (`gegenbauerC(3, a, x)`) are equal to
  MATLAB's but grouped differently. `poles` finds the poles of rational functions only
  (not those of `tan(x)`).
- Symbolic units cover about 100 common units with the SI prefixes, out of MATLAB's larger
  list; conversion to a unit system supports SI only, `unitInfo` lists only the units
  MatLivre knows, and `simplify` does not combine terms with different units of the same
  dimension (`2*[m] + 300*[cm]`).
- The Z, Fourier and Hilbert transforms cover the common transform pairs (polynomials and
  exponentials, sines and cosines, Gaussians, Dirac and Heaviside functions); other inputs
  stay unevaluated where MATLAB finds a closed form. `iztrans` needs rational poles, and
  `smithForm` and `hermiteForm` work on integer matrices only.
- `dsolve` solves single linear equations (constant coefficients, or first order with
  variable coefficients) with initial conditions; systems and nonlinear equations are not
  solved, and the order of terms in some solutions differs from MATLAB's
  (`C1*exp(-t) + exp(t)/2`).
- Table custom properties (`addprop`) are kept and read back, but `T.Properties` shows as a
  struct rather than MATLAB's `TableProperties` object.
- `Tiff` reads and writes uncompressed, PackBits, LZW and Deflate data without a predictor;
  JPEG-compressed and BigTIFF files are not read, and mode `'w8'` writes a classic TIFF.
- `combine` handles the `powers`, `exp`, `log` (with `IgnoreAnalyticConstraints`) and
  `sincos` targets. `sympref` keeps every preference, but of them only `HeavisideAtOrigin`
  changes results.
- Chart layouts that MATLAB fits to the screen (word clouds, bubble clouds, automatic
  `binscatter` bins) are approximated, and `geobubble` draws no basemap.
- Dithering in `rgb2ind` and `dither` is plain Floyd-Steinberg, so individual pixels can
  differ from MATLAB's; `'nodither'` matches.
- Random numbers come from MATLAB's Mersenne Twister, so `rand`, `randi` and `randperm` give
  MATLAB's values for a seed, but `randn` with the default Ziggurat transform does not (the
  `Polar` and `Inversion` transforms of `RandStream` do), and `randperm(n, k)` differs when `k`
  is more than a fifth of `n`.
- `timer` callbacks run during `pause`, `wait` and `drawnow`, but not once a script has
  finished, since there is no idle event loop.
- `settings` holds only a few of MATLAB's product settings (`matlab.fonts.codefont`), personal
  values last for the session instead of being saved, and a group lists its settings in the
  order they were added rather than in MATLAB's order.
- Sparse matrices are stored densely: they behave and display as in MATLAB, but matrices
  with more than 2e8 elements are refused.
- Windows is the only tested platform.

## License

[MIT](LICENSE)
