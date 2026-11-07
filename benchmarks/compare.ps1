# Builds a Markdown comparison from results\matlab.txt, results\octave.txt, results\scilab.txt and
# results\matlivre.txt (lines "RESULT <case> <seconds>" and "CHECK <case> <value>").
$dir = Join-Path $PSScriptRoot "results"
$engines = "matlab", "octave", "scilab", "matlivre"
$times = @{}; $checks = @{}
foreach ($e in $engines) {
    $times[$e] = @{}; $checks[$e] = @{}
    $file = Join-Path $dir "$e.txt"
    if (-not (Test-Path $file)) { continue }
    foreach ($line in Get-Content $file) {
        if ($line -match '^RESULT (\S+) (\S+)') { $times[$e][$matches[1]] = [double]$matches[2] }
        if ($line -match '^CHECK (\S+) (\S+)') { $checks[$e][$matches[1]] = $matches[2] }
    }
}
$cases = $times["matlivre"].Keys | Sort-Object { [array]::IndexOf(@(Get-Content (Join-Path $dir "matlivre.txt") | Where-Object { $_ -like 'RESULT*' } | ForEach-Object { ($_ -split ' ')[1] }), $_) }
function ms($s) { if ($null -eq $s) { "-" } else { "{0:N2}" -f ($s * 1000) } }
function ratio($a, $b) { if ($null -eq $a -or $null -eq $b -or $b -eq 0) { "n/a" } else { $r = $a / $b; if ($r -ge 1) { "{0:N1}x slower" -f $r } else { "{0:N1}x faster" -f (1 / $r) } } }
$out = @("| Case | MATLAB (ms) | Octave (ms) | Scilab (ms) | MatLivre (ms) | MatLivre vs MATLAB | MatLivre vs Octave | MatLivre vs Scilab | Checksums |", "|---|---:|---:|---:|---:|---|---|---|---|")
$lvo = 0.0; $lvm = 0.0; $lvs = 0.0
foreach ($c in $cases) {
    $m = $times["matlab"][$c]; $o = $times["octave"][$c]; $s = $times["scilab"][$c]; $l = $times["matlivre"][$c]
    $same = ($checks["matlab"][$c] -eq $checks["matlivre"][$c]) -and ($checks["octave"][$c] -eq $checks["matlivre"][$c]) -and ($checks["scilab"][$c] -eq $checks["matlivre"][$c])
    $out += "| $c | $(ms $m) | $(ms $o) | $(ms $s) | $(ms $l) | $(ratio $l $m) | $(ratio $l $o) | $(ratio $l $s) | $(if ($same) { 'match' } else { 'DIFFER' }) |"
    if ($o) { $lvo += [math]::Log($l / $o) }
    if ($m) { $lvm += [math]::Log($l / $m) }
    if ($s) { $lvs += [math]::Log($l / $s) }
}
$n = $cases.Count
$out += ""
$out += "Geometric mean over $n cases: matlivre is $(ratio ([math]::Exp($lvo / $n)) 1) than Octave, $(ratio ([math]::Exp($lvm / $n)) 1) than MATLAB and $(ratio ([math]::Exp($lvs / $n)) 1) than Scilab."
$out -join "`n"
