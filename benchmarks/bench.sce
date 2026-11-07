// Scilab port of bench.m. Same 12 cases, same sizes, same output format
// ("RESULT <name> <seconds>" and "CHECK <name> <value>") so compare.ps1 can read it.
// Run: WScilex-cli.exe -nb -f bench.sce -quit   (or Scilex.exe -nw)

function r = bench_fib(n)
    if n < 2 then
        r = n;
    else
        r = bench_fib(n - 1) + bench_fib(n - 2);
    end
endfunction

function report(name, best, chk)
    mprintf("RESULT %s %.6f\n", name, best);
    mprintf("CHECK %s %s\n", name, msprintf("%.10g", chk));
endfunction

REPS = 3;

// 1. Scalar arithmetic in a for loop
best = %inf;
for r = 1:REPS
    tic();
    s = 0;
    for k = 1:2000000
        s = s + k * k;
    end
    best = min(best, toc());
end
report("scalar_loop", best, s);

// 2. While loop with branches (Collatz)
best = %inf;
for r = 1:REPS
    tic();
    total = 0;
    for n = 1:30000
        m = n;
        while m <> 1
            if modulo(m, 2) == 0 then
                m = m / 2;
            else
                m = 3 * m + 1;
            end
            total = total + 1;
        end
    end
    best = min(best, toc());
end
report("while_branch", best, total);

// 3. Recursive function calls
best = %inf;
for r = 1:REPS
    tic();
    f = bench_fib(20);
    best = min(best, toc());
end
report("fib_recursive", best, f);

// 4. Growing an array element by element
best = %inf;
for r = 1:REPS
    tic();
    v = [];
    for k = 1:100000
        v($ + 1) = k;
    end
    best = min(best, toc());
end
report("array_growth", best, sum(v));

// 5. Indexed assignment into a preallocated array
best = %inf;
for r = 1:REPS
    tic();
    x = zeros(1, 1000000);
    for k = 1:1000000
        x(k) = k / 2;
    end
    best = min(best, toc());
end
report("indexed_assign", best, sum(x));

// 6. Vectorized elementwise math
best = %inf;
for r = 1:REPS
    tic();
    x = linspace(0, 10, 2000000);
    y = sin(x).^2 + cos(x).^2 + sqrt(x) .* exp(-x);
    s = sum(y);
    best = min(best, toc());
end
report("vector_math", best, s);

// 7. Dense matrix multiply (300x300)
A = matrix(modulo((1:90000) * 7, 13), 300, 300) / 13;
best = %inf;
for r = 1:REPS
    tic();
    B = A * A;
    best = min(best, toc());
end
report("matmul_300", best, sum(B));

// 8. Linear solve (300x300)
M = A + 300 * eye(300, 300);
b = ones(300, 1);
best = %inf;
for r = 1:REPS
    tic();
    sol = M \ b;
    best = min(best, toc());
end
report("linsolve_300", best, sum(sol));

// 9. Sorting a million numbers
data = modulo((1:1000000) * 7919, 1000003);
best = %inf;
for r = 1:REPS
    tic();
    srt = gsort(data, "g", "i");
    best = min(best, toc());
end
report("sort_1e6", best, srt(500000));

// 10. Building a string in a loop
best = %inf;
for r = 1:REPS
    tic();
    str = "";
    for k = 1:20000
        str = str + msprintf("%d,", k);
    end
    best = min(best, toc());
end
report("string_build", best, length(str));

// 11. Struct field updates
best = %inf;
for r = 1:REPS
    tic();
    p = struct("x", 0, "y", 0);
    for k = 1:500000
        p.x = p.x + 1;
        p.y = p.y + p.x;
    end
    best = min(best, toc());
end
report("struct_fields", best, p.y);

// 12. Cell array writes and reads
best = %inf;
for r = 1:REPS
    tic();
    c = cell(1, 100000);
    for k = 1:100000
        c{k} = k;
    end
    t = 0;
    for k = 1:100000
        t = t + c{k};
    end
    best = min(best, toc());
end
report("cell_ops", best, t);
