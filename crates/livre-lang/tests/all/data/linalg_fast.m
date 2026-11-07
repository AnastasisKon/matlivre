n = 80; A = reshape(mod((1:n*n)*7919 + 13*(1:n*n).^2, 1013), n, n) / 1013; S = A + A' + n*eye(n);
e = eig(S); fprintf('eigS %.8g %.8g %.8g\n', e(1), e(end), sum(e));
[V, D] = eig(S); fprintf('eigV %d %.8g\n', norm(S*V - V*D) < 1e-9, sum(diag(D)));
e2 = eig(A); fprintf('eigA %d %.6g %.6g\n', numel(e2), real(sum(e2)), max(abs(e2)));
[Q, R] = qr(A); fprintf('qr %d %d %d\n', norm(Q*R - A) < 1e-9, norm(Q'*Q - eye(n)) < 1e-9, sum(diag(R) < 0));
[Q, R] = qr(A(:, 1:40), 0); fprintf('qre %d %d %d %d\n', size(Q, 1), size(Q, 2), norm(Q*R - A(:, 1:40)) < 1e-9, sum(diag(R) < 0));
R = chol(S); fprintf('chol %d %.8g\n', norm(R'*R - S) < 1e-9, sum(diag(R)));
[R, p] = chol(-S); fprintf('cholfail %d\n', p);
[L, U, P] = lu(A); fprintf('lu %d %.8g\n', norm(P*A - L*U) < 1e-9, sum(log(abs(diag(U)))));
fprintf('norm %.8g\n', norm(A)); fprintf('rank %d\n', rank(A(:, 1:50) * A(1:50, :)));
c = conv(A(1, :), A(2, :)); fprintf('conv %.8g %d\n', sum(c), numel(c));
x = floor(A(:) * 50); y = floor(A(1:30, 1:30) * 50); [tf, loc] = ismember(x, y(:)); fprintf('ismember %d %d\n', sum(tf), sum(loc));
[u, ia, ic] = unique(x); fprintf('unique %d %d %d\n', numel(u), sum(ia), sum(ic));
[tf, loc] = ismember([1 NaN 0 -0], [NaN 0 1]); fprintf('ismember2 %d%d%d%d %d%d%d%d\n', tf, loc);
[tf, loc] = ismember([1 2; 3 4; 1 2], [3 4; 1 2; 1 2], 'rows'); fprintf('ismember3 %d%d%d %d%d%d\n', tf, loc);
m = containers.Map('KeyType', 'double', 'ValueType', 'double'); for k = [5 3 9 1 7 3], m(k) = k*2; end
fprintf('map %d %s %d %d\n', m.Count, mat2str(cell2mat(keys(m))), m(3), m.isKey(9)); remove(m, 3); fprintf('map2 %d %d\n', m.Count, m.isKey(3));
s = containers.Map(); s('b') = 1; s('a') = 2; s('c') = 3; s('a') = 20; fprintf('smap %d %s %d\n', s.Count, strjoin(keys(s), ','), s('a'));
z = sin(1:400); c2 = conv(z, cos(1:300), 'same'); fprintf('convsame %.8g %d\n', sum(c2), numel(c2));
