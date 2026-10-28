# Builds "Algebraic Fractals.exe" (the app) and algfrac.exe (the command line), and zips
# them with the user guide into dist\AlgebraicFractals-<version>-windows-x64.zip.
# Usage, from this folder:   powershell -ExecutionPolicy Bypass -File build.ps1
#
# Puts cargo and the MinGW-w64 tools (WinLibs, installed with winget) on PATH.
# The GUI's Windows dependencies need MinGW's dlltool and assembler, which the
# minimal Rust GNU toolchain doesn't include.
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$winlibs = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Directory -Filter "BrechtSanders.WinLibs*" -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path $_.FullName "mingw64\bin" } | Where-Object { Test-Path (Join-Path $_ "dlltool.exe") } | Select-Object -First 1
if ($winlibs) {
    $env:PATH = "$winlibs;$env:PATH"
} else {
    Write-Warning "MinGW-w64 not found. Install it with: winget install BrechtSanders.WinLibs.POSIX.MSVCRT --scope user"
}
Set-Location $PSScriptRoot
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# dist\ holds the copies to share: each .exe is self-contained (only system DLLs)
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$dist = "$PSScriptRoot\dist"
Remove-Item -Recurse -Force $dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item "$PSScriptRoot\target\release\algebraic-fractals.exe" "$dist\Algebraic Fractals.exe"
Copy-Item "$PSScriptRoot\target\release\algfrac.exe", "$PSScriptRoot\docs\USER-GUIDE.md" $dist
$zip = "$dist\AlgebraicFractals-$version-windows-x64.zip"
Compress-Archive -Path "$dist\Algebraic Fractals.exe", "$dist\algfrac.exe", "$dist\USER-GUIDE.md" -DestinationPath $zip -Force
Write-Host "`nBuilt (shareable copies in dist\):"
Write-Host "  $dist\Algebraic Fractals.exe   (double-click to explore and make videos)"
Write-Host "  $dist\algfrac.exe              (command line)"
Write-Host "  $zip"
exit 0
