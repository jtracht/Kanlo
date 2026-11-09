param([string]$Branch = 'main')
$ErrorActionPreference = 'Stop'
if ($Branch.StartsWith('-')) { throw 'Invalid branch name' }
git check-ref-format --branch $Branch | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Invalid branch name' }
$localCommit = git rev-parse --verify "refs/heads/$Branch"
if ($LASTEXITCODE -ne 0) { throw 'Local branch not found' }
foreach ($remote in @('origin', 'github')) {
    git push $remote "refs/heads/${Branch}:refs/heads/$Branch"
    if ($LASTEXITCODE -ne 0) { throw "Push to $remote failed; mirror is not confirmed" }
    $remoteRef = git ls-remote --exit-code $remote "refs/heads/$Branch"
    if ($LASTEXITCODE -ne 0) { throw "Verification of $remote failed" }
    $remoteCommit = ($remoteRef -split '\s+')[0]
    if ($remoteCommit -ne $localCommit) { throw "Commit mismatch on $remote" }
    Write-Output "$remote verified: $remoteCommit"
}
