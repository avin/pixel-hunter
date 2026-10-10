$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$native = Join-Path $repo 'src-tauri/native'
$version = '21.1.8'
$archive = Join-Path $native "downloads/LLVM-$version-win64.exe"
$destination = Join-Path $native "llvm-$version"
if (Test-Path (Join-Path $destination 'bin/libclang.dll')) { return }
Get-Command 7z -ErrorAction Stop | Out-Null
New-Item -ItemType Directory -Force (Split-Path $archive) | Out-Null
if (!(Test-Path $archive)) {
    Invoke-WebRequest "https://github.com/llvm/llvm-project/releases/download/llvmorg-$version/LLVM-$version-win64.exe" -OutFile $archive
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne '7A5386C26497DB1691F320121E5B113364DD0274B98E55F15F4DBC00C0450113') {
    throw 'LLVM source package SHA256 does not match the pinned release.'
}
& 7z x $archive "-o$destination" -y -bso0 -bsp0
if ($LASTEXITCODE) { throw 'LLVM extraction failed.' }
if (!(Test-Path (Join-Path $destination 'bin/libclang.dll'))) { throw 'LLVM package is missing libclang.dll.' }
