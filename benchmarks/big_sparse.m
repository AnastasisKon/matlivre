n = 1e6; e = ones(n,1); A = spdiags([-e 2*e -e], -1:1, n, n); b = A*ones(n,1);
tic; x = A\b; fprintf('tridiag n=1e6 solve: %.3f s, mem %.0f MB\n', toc, 3*n*16/1e6);
m = 500; I = speye(m); T = spdiags([-ones(m,1) 2*ones(m,1) -ones(m,1)], -1:1, m, m); P = kron(I,T)+kron(T,I);
tic; u = P\ones(m*m,1); fprintf('poisson 250k unknowns solve: %.3f s\n', toc);
