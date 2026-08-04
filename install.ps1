<#
.SYNOPSIS
  ckit standalone installer for Windows (PowerShell) — PUBLIC GitHub repo.

.DESCRIPTION
  Downloads the prebuilt `ckit.exe` binary from the latest GitHub Release of
  the public repo dante241/ckit_agent. No token required.

  One-liner:
    irm https://raw.githubusercontent.com/dante241/ckit_agent/main/install.ps1 | iex

  Upgrade:   re-run the same command (atomically replaces the old binary).
  Uninstall: download the script and run:  .\install.ps1 -Uninstall

.PARAMETER Uninstall
  Remove the installed ckit.exe and exit.

.NOTES
  Environment:
    CKIT_GITHUB_TOKEN  optional — only to dodge GitHub's 60-req/hour anonymous
                       API limit. GITHUB_TOKEN also accepted.
    CKIT_VERSION       release tag to install (default: latest, e.g. v0.53.0)
    CKIT_BIN_DIR       install location (default: %LOCALAPPDATA%\Programs\ckit)
#>
param(
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'

$Repo = 'dante241/ckit_agent'
$Api = "https://api.github.com/repos/$Repo"
$BinDir = if ($env:CKIT_BIN_DIR) { $env:CKIT_BIN_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\ckit' }
$BinName = 'ckit.exe'
$BinPath = Join-Path $BinDir $BinName

# --- uninstall -------------------------------------------------------------

if ($Uninstall) {
    if (Test-Path -LiteralPath $BinPath) {
        Remove-Item -LiteralPath $BinPath -Force
        Write-Host "ckit uninstalled (removed $BinPath)."
    } else {
        Write-Host "ckit not found at $BinPath; nothing to uninstall."
    }
    return
}

# Ensure TLS 1.2 on older Windows PowerShell (5.1 defaults can be too weak).
try {
    [Net.ServicePointManager]::SecurityProtocol = `
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {}

# Optional token — public repo needs none; only lifts the anon API rate limit.
$token = if ($env:CKIT_GITHUB_TOKEN) { $env:CKIT_GITHUB_TOKEN } elseif ($env:GITHUB_TOKEN) { $env:GITHUB_TOKEN } else { $null }
$headers = @{ 'Accept' = 'application/vnd.github+json'; 'User-Agent' = 'ckit-installer' }
if ($token) { $headers['Authorization'] = "Bearer $token" }

# --- resolve release -------------------------------------------------------

$version = $env:CKIT_VERSION
if ($version) {
    if ($version -notlike 'v*') { $version = "v$version" }
    $relUrl = "$Api/releases/tags/$version"
} else {
    $relUrl = "$Api/releases/latest"
}
try {
    $release = Invoke-RestMethod -Uri $relUrl -Headers $headers -UseBasicParsing
} catch {
    throw "ckit: could not query release ($relUrl)`n$($_.Exception.Message)"
}
$tag = $release.tag_name
if (-not $tag) { throw "ckit: could not resolve release tag." }

$asset = "ckit-$tag-windows-x86_64.exe"
$assetObj = $release.assets | Where-Object { $_.name -eq $asset } | Select-Object -First 1
if (-not $assetObj) { throw "ckit: release $tag has no asset '$asset'." }

# --- download + install ----------------------------------------------------

Write-Host "Installing ckit $tag (windows-x86_64)..."
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

# Public repo → the browser_download_url needs no auth.
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("ckit-" + [System.Guid]::NewGuid().ToString('N') + ".exe")
try {
    Invoke-WebRequest -Uri $assetObj.browser_download_url -OutFile $tmp -UseBasicParsing -Headers @{ 'User-Agent' = 'ckit-installer' }
} catch {
    if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue }
    throw "ckit: download failed: $($assetObj.browser_download_url)`n$($_.Exception.Message)"
}
if (-not (Test-Path -LiteralPath $tmp) -or (Get-Item -LiteralPath $tmp).Length -eq 0) {
    if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue }
    throw "ckit: downloaded an empty file from $($assetObj.browser_download_url)"
}

Move-Item -LiteralPath $tmp -Destination $BinPath -Force

Write-Host "Installed -> $BinPath"
try { & $BinPath --version } catch {}

# --- PATH ------------------------------------------------------------------

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$segments = @()
if ($userPath) { $segments = $userPath -split ';' | Where-Object { $_ -ne '' } }
$already = $segments | Where-Object { $_.TrimEnd('\') -ieq $BinDir.TrimEnd('\') }
if (-not $already) {
    $newPath = if ($userPath) { "$userPath;$BinDir" } else { $BinDir }
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    $env:Path = "$env:Path;$BinDir"
    Write-Host ""
    Write-Host "$BinDir added to your user PATH."
    Write-Host "Restart your shell (or open a new terminal) for the change to take effect."
}

# --- next steps ------------------------------------------------------------

Write-Host ""
Write-Host "Done. Next steps:"
Write-Host "  ckit setup        # install the AI core (omp + codegraph + MCP servers)"
Write-Host "  ckit doctor       # verify"
Write-Host "  ckit up           # upgrade later (or re-run this installer)"
Write-Host ""
Write-Host "Before 'ckit setup' on Windows, make sure these are on PATH:"
Write-Host "  - bun (https://bun.sh) or Node.js npm   # installs omp + codegraph"
Write-Host "  - winget (built into Windows 10/11)      # installs GitHub CLI (gh)"
Write-Host "  - uv (optional, https://astral.sh/uv)    # headroom + serena MCP servers"
