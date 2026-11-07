% Sparse and heavy vectorized benchmark. Runs in MATLAB and matlivre.
% Prints "RESULT <name> <seconds>" (best of REPS) and "CHECK <name> <value>".
% Sparse matrices are stored densely in matlivre, so sizes are kept to what a dense array allows.
REPS = 3;

%% sparse tridiagonal solve, n = 3000
n = 3000; e = ones(n,1);
A = spdiags([-e 2*e -e], -1:1, n, n); b = A * (1:n)';
best = inf; for r = 1:REPS, tic; x = A \ b; best = min(best, toc); end
fprintf('RESULT sp_tridiag_solve_3000 %.6f\n', best); fprintf('CHECK sp_tridiag_solve_3000 %.6g\n', sum(x));

%% 2-D Poisson (5-point), 50x50 grid = 2500 unknowns, sparse solve
m = 50; I = speye(m); T = spdiags([-ones(m,1) 2*ones(m,1) -ones(m,1)], -1:1, m, m);
P = kron(I, T) + kron(T, I); f = ones(m*m, 1);
best = inf; for r = 1:REPS, tic; u = P \ f; best = min(best, toc); end
fprintf('RESULT sp_poisson2d_solve_2500 %.6f\n', best); fprintf('CHECK sp_poisson2d_solve_2500 %.6g\n', sum(u));

%% sparse matrix-vector product, repeated
n = 3000; e = ones(n,1); A = spdiags([-e 2*e -e], -1:1, n, n); v = (1:n)' / n;
best = inf; for r = 1:REPS, tic; w = v; for k = 1:200, w = A * w; w = w / norm(w); end; best = min(best, toc); end
fprintf('RESULT sp_matvec_200x_3000 %.6f\n', best); fprintf('CHECK sp_matvec_200x_3000 %.6g\n', sum(w));

%% sparse construction from triplets
n = 2000; i = [1:n, 1:n-1, 2:n]; j = [1:n, 2:n, 1:n-1]; s = [2*ones(1,n), -ones(1,2*(n-1))];
best = inf; for r = 1:REPS, tic; for k = 1:20, S = sparse(i, j, s, n, n); end; best = min(best, toc); end
fprintf('RESULT sp_build_20x_2000 %.6f\n', best); fprintf('CHECK sp_build_20x_2000 %.6g\n', nnz(S));

%% conjugate gradients on the 2-D Poisson matrix
best = inf; for r = 1:REPS, tic; [u2, flag] = pcg(P, f, 1e-8, 500); best = min(best, toc); end
fprintf('RESULT sp_pcg_poisson_2500 %.6f\n', best); fprintf('CHECK sp_pcg_poisson_2500 %.4g\n', norm(P*u2 - f) < 1e-5);

%% heavy vectorized operations (1e7 elements)
N = 1e7; x = linspace(0, 1, N);
best = inf; for r = 1:REPS, tic; y = exp(-x.^2) .* sin(20*x) + sqrt(x + 1); s = sum(y); best = min(best, toc); end
fprintf('RESULT vec_elementwise_1e7 %.6f\n', best); fprintf('CHECK vec_elementwise_1e7 %.8g\n', s);
best = inf; for r = 1:REPS, tic; c = cumsum(x); m = max(c) - min(c); best = min(best, toc); end
fprintf('RESULT vec_cumsum_1e7 %.6f\n', best); fprintf('CHECK vec_cumsum_1e7 %.8g\n', m);
best = inf; for r = 1:REPS, tic; k = find(x > 0.25 & x < 0.75); q = numel(k); best = min(best, toc); end
fprintf('RESULT vec_find_logical_1e7 %.6f\n', best); fprintf('CHECK vec_find_logical_1e7 %d\n', q);
M = reshape(mod((1:4e6) * 7, 1013), 2000, 2000) / 1013; rv = (1:2000) / 2000; cv = (1:2000)' / 2000;
best = inf; for r = 1:REPS, tic; Z = M .* rv + cv - M; s = sum(Z(:)); best = min(best, toc); end
fprintf('RESULT vec_broadcast_2000sq %.6f\n', best); fprintf('CHECK vec_broadcast_2000sq %.8g\n', s);
best = inf; for r = 1:REPS, tic; cs = sum(M, 1); rs = sum(M, 2); mx = max(M, [], 2); best = min(best, toc); end
fprintf('RESULT vec_reductions_2000sq %.6f\n', best); fprintf('CHECK vec_reductions_2000sq %.8g\n', sum(cs) + sum(rs) + sum(mx));
best = inf; for r = 1:REPS, tic; sv = sort(M(:)); best = min(best, toc); end
fprintf('RESULT vec_sort_4e6 %.6f\n', best); fprintf('CHECK vec_sort_4e6 %.8g\n', sv(2e6));
best = inf; for r = 1:REPS, tic; MM = M * M; best = min(best, toc); end
fprintf('RESULT dense_matmul_2000 %.6f\n', best); fprintf('CHECK dense_matmul_2000 %.6g\n', sum(MM(:)));
