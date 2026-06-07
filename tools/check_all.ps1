$ErrorActionPreference = "Stop"

function Invoke-Checked {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Name,
        [Parameter(Mandatory=$true)]
        [string]$File,
        [string[]]$Arguments = @()
    )

    & $File @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $RepoRoot
try {
    Invoke-Checked "package validation" "python" @("tools\check_packages.py", "--repo-root", ".")
    Invoke-Checked "python compile" "python" @(
        "-m",
        "py_compile",
        "tools\check_packages.py",
        "tools\hand_animation_matter_bridge.py",
        "tools\package_testkit.py",
        "tools\check_device_readiness.py"
    )
    Invoke-Checked "cargo fmt" "cargo" @("fmt", "--all", "--check")
    Invoke-Checked "cargo test" "cargo" @("test", "--workspace")
    Invoke-Checked "polar goldens" "cargo" @(
        "run",
        "-p",
        "polar-h10-core",
        "--",
        "validate-goldens",
        "--package-root",
        "packages\polar-h10"
    )
    Invoke-Checked "projected motion breath goldens" "cargo" @(
        "run",
        "-p",
        "projected-motion-breath-core",
        "--",
        "validate-goldens",
        "--package-root",
        "packages\projected-motion-breath"
    )
    Invoke-Checked "projected motion breath live route self-test" "cargo" @(
        "run",
        "-p",
        "projected-motion-breath-core",
        "--",
        "live-route-self-test",
        "--package-root",
        "packages\projected-motion-breath"
    )
    Invoke-Checked "desktop readiness" "python" @("tools\check_device_readiness.py", "--repo-root", ".", "--host-profile", "desktop")
    Invoke-Checked "mobile readiness" "python" @("tools\check_device_readiness.py", "--repo-root", ".", "--host-profile", "mobile")
    Invoke-Checked "headset readiness" "python" @("tools\check_device_readiness.py", "--repo-root", ".", "--host-profile", "headset")
} finally {
    Pop-Location
}
