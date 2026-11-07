% Cases 7 and 8 of bench.m (300x300 product and solve), repeated so the spread is visible.
A = reshape(mod((1:90000) * 7, 13), 300, 300) / 13;
M = A + 300 * eye(300); b = ones(300, 1);
for round = 1:5
    best = inf; for r = 1:3, tic; B = A * A; best = min(best, toc); end; mm = best;
    best = inf; for r = 1:3, tic; sol = M \ b; best = min(best, toc); end; ls = best;
    fprintf('matmul_300 %.3f ms   linsolve_300 %.3f ms\n', mm*1e3, ls*1e3);
end
