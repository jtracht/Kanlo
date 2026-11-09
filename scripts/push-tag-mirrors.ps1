param([Parameter(Mandatory=$true)][string]$Tag)
$ErrorActionPreference = 'Stop'
if ($Tag -notmatch '^v\d+\.\d+\.\d+(-(dev|beta|rc)\.\d+)?$') { throw 'Expected a version tag' }
$tagObject = git rev-parse --verify "refs/tags/$Tag"
if ($LASTEXITCODE -ne 0) { throw 'Local tag does not exist' }
foreach ($remote in @('origin','github')) {
    git push $remote "refs/tags/${Tag}:refs/tags/$Tag"
    $pushResult = $LASTEXITCODE
    $remoteRef = git ls-remote --exit-code $remote "refs/tags/$Tag"
    if ($LASTEXITCODE -ne 0) { throw "Cannot verify $remote tag" }
    if (($remoteRef -split '\s+')[0] -ne $tagObject) { throw "Different tag object on $remote; no overwrite attempted" }
    if ($pushResult -ne 0) { Write-Output "$remote already contains the exact tag object; concurrent mirror verified" }
    Write-Output "$remote tag verified: $Tag -> $tagObject"
}
