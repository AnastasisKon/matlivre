// Scilab port of bench_sparse.m (same sizes). Differences forced by the language:
//  - sparse matrices are built with sparse([i j], v, [m n]) (no spdiags),
//  - there is no implicit expansion, so the broadcast case multiplies by ones() products,
//  - there is no pcg: a plain conjugate-gradient loop (tolerance 1e-8, at most 500 iterations).
// Prints "RESULT <name> <seconds>" (best of REPS) and "CHECK <name> <value>".
REPS = 3;

function report(name, best, chk)
    mprintf("RESULT %s %.6f\n", name, best);
    mprintf("CHECK %s %s\n", name, msprintf("%.6g", chk));
endfunction

function A = tridiag(n)
    i = [1:n, 1:n-1, 2:n]';
    j = [1:n, 2:n, 1:n-1]';
    v = [2*ones(1, n), -ones(1, 2*(n-1))]';
    A = sparse([i j], v, [n n]);
endfunction

function [x, it] = cg(A, b, tol, maxit)
    x = zeros(b); r = b - A*x; p = r; rs = r'*r; nb = norm(b);
    for it = 1:maxit
        Ap = A*p; al = rs / (p'*Ap);
        x = x + al*p; r = r - al*Ap; rn = r'*r;
        if sqrt(rn) / nb <= tol then break; end
        p = r + (rn/rs)*p; rs = rn;
    end
endfunction

// sparse tridiagonal solve, n = 3000
n = 3000; A = tridiag(n); b = A * (1:n)';
best = %inf; for r = 1:REPS, tic(); x = A \ b; best = min(best, toc()); end
report("sp_tridiag_solve_3000", best, sum(x));

// 2-D Poisson, 50x50 grid
m = 50; I = speye(m, m);
e = ones(m, 1); T = sparse([[1:m 1:m-1 2:m]' [1:m 2:m 1:m-1]'], [2*e'  -ones(1, 2*(m-1))]', [m m]);
P = kron(I, T) + kron(T, I); f = ones(m*m, 1);
best = %inf; for r = 1:REPS, tic(); u = P \ f; best = min(best, toc()); end
report("sp_poisson2d_solve_2500", best, sum(u));

// sparse matrix-vector products
n = 3000; A = tridiag(n); v = (1:n)' / n;
best = %inf;
for r = 1:REPS
    tic(); w = v;
    for k = 1:200, w = A * w; w = w / norm(w); end
    best = min(best, toc());
end
report("sp_matvec_200x_3000", best, sum(w));

// construction from triplets
n = 2000; ii = [1:n, 1:n-1, 2:n]'; jj = [1:n, 2:n, 1:n-1]'; ss = [2*ones(1, n), -ones(1, 2*(n-1))]';
best = %inf;
for r = 1:REPS
    tic();
    for k = 1:20, S = sparse([ii jj], ss, [n n]); end
    best = min(best, toc());
end
report("sp_build_20x_2000", best, nnz(S));

// conjugate gradients on the Poisson matrix
best = %inf; for r = 1:REPS, tic(); [u2, itc] = cg(P, f, 1e-8, 500); best = min(best, toc()); end
report("sp_pcg_poisson_2500", best, bool2s(norm(P*u2 - f) < 1e-5));

// heavy vectorized operations
N = 1e7; x = linspace(0, 1, N);
best = %inf; for r = 1:REPS, tic(); y = exp(-x.^2) .* sin(20*x) + sqrt(x + 1); s = sum(y); best = min(best, toc()); end
report("vec_elementwise_1e7", best, s);
best = %inf; for r = 1:REPS, tic(); c = cumsum(x); mm = max(c) - min(c); best = min(best, toc()); end
report("vec_cumsum_1e7", best, mm);
best = %inf; for r = 1:REPS, tic(); k = find(x > 0.25 & x < 0.75); q = size(k, "*"); best = min(best, toc()); end
report("vec_find_logical_1e7", best, q);
M = matrix(modulo((1:4e6) * 7, 1013), 2000, 2000) / 1013; rv = (1:2000) / 2000; cv = (1:2000)' / 2000;
best = %inf;
for r = 1:REPS
    tic(); Z = M .* (ones(2000, 1) * rv) + (cv * ones(1, 2000)) - M; s = sum(Z); best = min(best, toc());
end
report("vec_broadcast_2000sq", best, s);
best = %inf; for r = 1:REPS, tic(); cs = sum(M, 1); rs = sum(M, 2); mx = max(M, "c"); best = min(best, toc()); end
report("vec_reductions_2000sq", best, sum(cs) + sum(rs) + sum(mx));
best = %inf; for r = 1:REPS, tic(); sv = gsort(M(:), "g", "i"); best = min(best, toc()); end
report("vec_sort_4e6", best, sv(2e6));
best = %inf; for r = 1:REPS, tic(); MM = M * M; best = min(best, toc()); end
report("dense_matmul_2000", best, sum(MM));
