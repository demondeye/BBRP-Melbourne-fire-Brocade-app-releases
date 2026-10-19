# Publishes a new version of Melbourne Fire Brigade as a software update.
# The exe is attached to a release on the repo set in src/update.rs.
# Commit and push your code first so the release matches it.
#
# 1. Raise `version` in Cargo.toml (for example 0.2.0 -> 0.3.0).
# 2. Write what's new in RELEASE_NOTES.md.
# 3. Run:  powershell -ExecutionPolicy Bypass -File release.ps1
#
# Needs Rust (https://rustup.rs) and the GitHub CLI (https://cli.github.com),
# signed in once with:  gh auth login
# Every copy of the app sees the update within a few hours (or straight away
# with Settings -> Software Update -> Check for updates).

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$repo = (Select-String -Path "src\update.rs" -Pattern 'UPDATE_REPO: &str = "([^"]+)"').Matches[0].Groups[1].Value
$version = (Select-String -Path "Cargo.toml" -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$tag = "v$version"
Write-Host "Publishing Melbourne Fire Brigade $version to github.com/$repo" -ForegroundColor Cyan

$existing = gh release view $tag --repo $repo 2>$null
if ($LASTEXITCODE -eq 0) {
    throw "Version $version is already published. Raise the version in Cargo.toml first."
}

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "The build failed." }
Copy-Item "target\release\MelbourneFireBrigade.exe" "MelbourneFireBrigade.exe" -Force

$notes = "RELEASE_NOTES.md"
if (-not (Test-Path $notes)) { Set-Content $notes "Bug fixes and improvements." }

gh release create $tag "MelbourneFireBrigade.exe" --repo $repo --title "Melbourne Fire Brigade $version" --notes-file $notes
if ($LASTEXITCODE -ne 0) { throw "Publishing to GitHub failed." }
Write-Host "Done. Version $version is live as an update." -ForegroundColor Green
