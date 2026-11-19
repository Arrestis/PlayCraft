# Publish this project to GitHub as the "PlayCraft" repository.
#
# Run from PowerShell in the project folder:
#     powershell -ExecutionPolicy Bypass -File scripts\publish-to-github.ps1
#
# It installs Git and the GitHub CLI if needed (via winget), signs you in through GitHub's
# own web page (no password is typed here or stored), creates the repository and uploads the code.

param(
    [string]$Name = "PlayCraft",
    [ValidateSet("public", "private")]
    [string]$Visibility = "public"
)

$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

function Get-Tool([string]$exe, [string]$wingetId) {
    if (-not (Get-Command $exe -ErrorAction SilentlyContinue)) {
        Write-Host "Installing $wingetId ..."
        winget install --id $wingetId -e --accept-source-agreements --accept-package-agreements
        $env:PATH = [Environment]::GetEnvironmentVariable("PATH", "Machine") + ";" + [Environment]::GetEnvironmentVariable("PATH", "User")
    }
}

Get-Tool git "Git.Git"
Get-Tool gh "GitHub.cli"

# Sign in through the browser if not already signed in.
gh auth status 2>$null | Out-Null
if ($LASTEXITCODE -ne 0) {
    gh auth login --hostname github.com --git-protocol https --web
}

if (-not (Test-Path .git)) {
    git init -b main
}
if (-not (git config user.name)) { git config user.name (gh api user --jq .login) }
if (-not (git config user.email)) { git config user.email ((gh api user --jq .login) + "@users.noreply.github.com") }

git add -A
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) { git commit -m "PlayCraft: open-source DVD, CD and Blu-ray player" }

$owner = gh api user --jq .login
gh repo view "$owner/$Name" 2>$null | Out-Null
if ($LASTEXITCODE -ne 0) {
    gh repo create $Name "--$Visibility" --source . --remote origin --push `
        --description "Free, open-source DVD, CD and Blu-ray player for Windows, macOS and Linux"
} else {
    if (-not (git remote)) { git remote add origin "https://github.com/$owner/$Name.git" }
    git push -u origin main
}

Write-Host ""
Write-Host "Done: https://github.com/$owner/$Name"
Write-Host "To build Windows, Mac and Linux downloads, open the Actions tab there and run 'Build packages'."
