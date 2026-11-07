% Broader benchmark: common numerical and general-purpose operations. Runs in MATLAB and matlivre.
% Prints "RESULT <name> <seconds>" (best of REPS) and "CHECK <name> <value>".
REPS = 3;
rep = @(f) 0;
n = 300; A = reshape(mod((1:n*n)*7919, 1013), n, n) / 1013; S = A + A' + n*eye(n);
x6 = mod((1:2^20) * 7919, 10007) / 10007;

best = inf; for r = 1:REPS, tic; F = fft(x6); best = min(best, toc); end
fprintf('RESULT fft_2e20 %.6f\n', best); fprintf('CHECK fft_2e20 %.6g\n', sum(abs(F)));
best = inf; h = mod((1:1000), 17) / 17; xx = x6(1:100000); for r = 1:REPS, tic; c = conv(xx, h); best = min(best, toc); end
fprintf('RESULT conv_1e5x1e3 %.6f\n', best); fprintf('CHECK conv_1e5x1e3 %.6g\n', sum(c));
best = inf; for r = 1:REPS, tic; s = svd(A); best = min(best, toc); end
fprintf('RESULT svd_300 %.6f\n', best); fprintf('CHECK svd_300 %.6g\n', sum(s));
best = inf; for r = 1:REPS, tic; e = eig(S); best = min(best, toc); end
fprintf('RESULT eig_sym_300 %.6f\n', best); fprintf('CHECK eig_sym_300 %.6g\n', sum(e));
G = A(1:150, 1:150);
best = inf; for r = 1:REPS, tic; e = eig(G); best = min(best, toc); end
fprintf('RESULT eig_gen_150 %.6f\n', best); fprintf('CHECK eig_gen_150 %.6g\n', abs(sum(e)));
best = inf; for r = 1:REPS, tic; [Q, R] = qr(A); best = min(best, toc); end
fprintf('RESULT qr_300 %.6f\n', best); fprintf('CHECK qr_300 %.6g\n', abs(sum(diag(R))));
best = inf; for r = 1:REPS, tic; Ai = inv(S); best = min(best, toc); end
fprintf('RESULT inv_300 %.6f\n', best); fprintf('CHECK inv_300 %.6g\n', sum(Ai(:)));
best = inf; for r = 1:REPS, tic; L = chol(S); best = min(best, toc); end
fprintf('RESULT chol_300 %.6f\n', best); fprintf('CHECK chol_300 %.6g\n', sum(diag(L)));
best = inf; for r = 1:REPS, tic; [L2, U2, P2] = lu(A); best = min(best, toc); end
fprintf('RESULT lu_300 %.6f\n', best); fprintf('CHECK lu_300 %.6g\n', abs(sum(diag(U2))));
best = inf; for r = 1:REPS, tic; Pi = pinv(G); best = min(best, toc); end
fprintf('RESULT pinv_150 %.6f\n', best); fprintf('CHECK pinv_150 %.6g\n', abs(sum(Pi(:))));
best = inf; for r = 1:REPS, tic; E = expm(G / 100); best = min(best, toc); end
fprintf('RESULT expm_150 %.6f\n', best); fprintf('CHECK expm_150 %.6g\n', sum(E(:)));

c1 = num2cell(1:100000);
best = inf; for r = 1:REPS, tic; y = cellfun(@(v) v * 2, c1); best = min(best, toc); end
fprintf('RESULT cellfun_anon_1e5 %.6f\n', best); fprintf('CHECK cellfun_anon_1e5 %.6g\n', sum(y));
best = inf; for r = 1:REPS, tic; y = arrayfun(@(v) v^2, 1:100000); best = min(best, toc); end
fprintf('RESULT arrayfun_anon_1e5 %.6f\n', best); fprintf('CHECK arrayfun_anon_1e5 %.6g\n', sum(y));
words = cell(1, 20000); for k = 1:20000, words{k} = sprintf('w%d', mod(k * 31, 997)); end
best = inf; for r = 1:REPS, tic; j = strjoin(words, ','); p = strsplit(j, ','); best = min(best, toc); end
fprintf('RESULT strjoin_split_2e4 %.6f\n', best); fprintf('CHECK strjoin_split_2e4 %d\n', numel(p));
best = inf; for r = 1:REPS, tic; q = regexprep(j, 'w(\d)', 'W$1'); u = upper(strrep(q, ',', ';')); best = min(best, toc); end
fprintf('RESULT regexprep_upper %.6f\n', best); fprintf('CHECK regexprep_upper %d\n', length(u));
best = inf; for r = 1:REPS, tic; for k = 1:10000, t = num2str(k * 1.5); end; best = min(best, toc); end
fprintf('RESULT num2str_1e4 %.6f\n', best); fprintf('CHECK num2str_1e4 %s\n', t);
best = inf; for r = 1:REPS, tic; [uq, ia, ic] = unique(floor(x6 * 5000)); best = min(best, toc); end
fprintf('RESULT unique_1e6 %.6f\n', best); fprintf('CHECK unique_1e6 %d\n', numel(uq));
a1 = floor(x6(1:100000) * 100000); b1 = floor(x6(100001:200000) * 100000);
best = inf; for r = 1:REPS, tic; tf = ismember(a1, b1); best = min(best, toc); end
fprintf('RESULT ismember_1e5 %.6f\n', best); fprintf('CHECK ismember_1e5 %d\n', sum(tf));
best = inf; for r = 1:REPS, tic; [ss, ii] = sort(x6); best = min(best, toc); end
fprintf('RESULT sort_idx_1e6 %.6f\n', best); fprintf('CHECK sort_idx_1e6 %d\n', ii(1000));
T = [floor(x6(1:100000) * 100)' floor(x6(100001:200000) * 100)' (1:100000)'];
best = inf; for r = 1:REPS, tic; T2 = sortrows(T, [1 2]); best = min(best, toc); end
fprintf('RESULT sortrows_1e5 %.6f\n', best); fprintf('CHECK sortrows_1e5 %d\n', T2(5000, 3));
best = inf; for r = 1:REPS, tic; z = x6; z(z > 0.5) = 0; z(z < 0.1) = -1; best = min(best, toc); end
fprintf('RESULT mask_assign_1e6 %.6f\n', best); fprintf('CHECK mask_assign_1e6 %.8g\n', sum(z));
best = inf; for r = 1:REPS, tic; t = 0; for k = 1:n, col = A(:, k); t = t + sum(col .* col); end; best = min(best, toc); end
fprintf('RESULT column_loop_300 %.6f\n', best); fprintf('CHECK column_loop_300 %.8g\n', t);
best = inf; for r = 1:REPS, tic; for k = 1:300, B = A(1:150, 1:150); B(:, 1) = k; end; best = min(best, toc); end
fprintf('RESULT submatrix_copy %.6f\n', best); fprintf('CHECK submatrix_copy %.8g\n', sum(B(:)));
V = reshape(1:1e6, 100, 100, 100);
best = inf; for r = 1:REPS, tic; W = permute(V, [3 1 2]); W2 = repmat(A(1:100, 1:100), 5, 5); best = min(best, toc); end
fprintf('RESULT permute_repmat %.6f\n', best); fprintf('CHECK permute_repmat %.8g\n', W(7, 8, 9) + sum(W2(:)));
best = inf; for r = 1:REPS, tic; ac = accumarray(floor(x6' * 1000) + 1, 1); best = min(best, toc); end
fprintf('RESULT accumarray_1e6 %.6f\n', best); fprintf('CHECK accumarray_1e6 %d\n', max(ac));
best = inf; xq = linspace(0, 1, 1e6); for r = 1:REPS, tic; yi = interp1(linspace(0, 1, 1000), mod(1:1000, 7), xq); best = min(best, toc); end
fprintf('RESULT interp1_1e6 %.6f\n', best); fprintf('CHECK interp1_1e6 %.8g\n', sum(yi));
best = inf; for r = 1:REPS, tic; pc = polyfit(xq(1:100000), sin(xq(1:100000)), 5); pv = polyval(pc, xq); best = min(best, toc); end
fprintf('RESULT polyfit_polyval %.6f\n', best); fprintf('CHECK polyfit_polyval %.6g\n', sum(pv));
best = inf; for r = 1:REPS, tic; m1 = mean(x6); s1 = std(x6); md = median(x6); v1 = var(x6); best = min(best, toc); end
fprintf('RESULT stats_1e6 %.6f\n', best); fprintf('CHECK stats_1e6 %.8g\n', m1 + s1 + md + v1);
best = inf; for r = 1:REPS, tic; st = struct('a', num2cell(1:50000)); t = 0; for k = 1:50000, t = t + st(k).a; end; best = min(best, toc); end
fprintf('RESULT struct_array_5e4 %.6f\n', best); fprintf('CHECK struct_array_5e4 %d\n', t);
best = inf; for r = 1:REPS, tic; mp = containers.Map('KeyType', 'double', 'ValueType', 'double'); for k = 1:5000, mp(k) = k * 2; end; t = 0; for k = 1:5000, t = t + mp(k); end; best = min(best, toc); end
fprintf('RESULT map_5e3 %.6f\n', best); fprintf('CHECK map_5e3 %d\n', t);
best = inf; for r = 1:REPS, tic; rr = zeros(1, 20000); for k = 1:20000, rr(k) = addtwo(k, 2); end; best = min(best, toc); end
fprintf('RESULT user_func_calls %.6f\n', best); fprintf('CHECK user_func_calls %d\n', sum(rr));
best = inf; for r = 1:REPS, tic; K = kron(A(1:30, 1:30), A(1:30, 1:30)); N = norm(A); R = rank(G); best = min(best, toc); end
fprintf('RESULT kron_norm_rank %.6f\n', best); fprintf('CHECK kron_norm_rank %.6g\n', sum(K(:)) + N + R);
best = inf; for r = 1:REPS, tic; q = rand(1, 1e7); w = randn(1000, 1000); best = min(best, toc); end
fprintf('RESULT rand_gen %.6f\n', best); fprintf('CHECK rand_gen %d\n', numel(q) + numel(w));
best = inf; for r = 1:REPS, tic; d1 = diff(x6); cp = cumprod(1 + d1 / 1000); tz = trapz(x6); best = min(best, toc); end
fprintf('RESULT diff_cumprod_trapz %.6f\n', best); fprintf('CHECK diff_cumprod_trapz %.8g\n', cp(end) + tz);

function r = addtwo(a, b)
    r = a + b;
end
