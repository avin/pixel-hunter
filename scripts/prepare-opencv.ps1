param([string]$LlvmBin)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$native = Join-Path $repo 'src-tauri/native'
$version = '4.12.0'
$sha256 = 'FA3FAF7581F1FA943C9E670CF57DD6BA1C5B4178F363A188A2C8BFF1EB28B7E4'
$archive = Join-Path $native "downloads/opencv-$version.zip"
$source = Join-Path $native "opencv-$version"
$build = Join-Path $native 'build'
$install = Join-Path $native 'install'
$runtime = Join-Path $native 'runtime'

if (!$LlvmBin) {
    & (Join-Path $PSScriptRoot 'prepare-llvm.ps1')
    $LlvmBin = Join-Path $native 'llvm-21.1.8/bin'
}

if (!(Test-Path (Join-Path $LlvmBin 'libclang.dll'))) { throw "Install LLVM with libclang.dll, or pass -LlvmBin." }
$env:LIBCLANG_PATH = $LlvmBin
$env:CLANG_PATH = Join-Path $LlvmBin 'clang.exe'
foreach ($tool in @('cmake', 'ninja', 'cargo')) { Get-Command $tool -ErrorAction Stop | Out-Null }
$vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vs) { throw 'Install Visual Studio C++ desktop development with the Windows SDK.' }
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvars64.bat'
# Import the x64 MSVC compiler environment into this shell, including SDK include paths.
& $env:ComSpec /d /s /c "`"$vcvars`" >nul && set" | ForEach-Object {
    if ($_ -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process') }
}
if ($LASTEXITCODE) { throw 'Could not initialize the MSVC environment.' }
New-Item -ItemType Directory -Force (Split-Path $archive), $runtime | Out-Null
if (!(Test-Path $archive)) {
    Invoke-WebRequest "https://github.com/opencv/opencv/archive/refs/tags/$version.zip" -OutFile $archive
}
if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne $sha256) { throw 'OpenCV source SHA256 does not match the pinned release.' }
if (!(Test-Path $source)) { Expand-Archive -LiteralPath $archive -DestinationPath $native }
& cmake -S $source -B $build -G Ninja "-DCMAKE_INSTALL_PREFIX=$install" -DCMAKE_BUILD_TYPE=Release `
    -DOPENCV_LIB_INSTALL_PATH=lib -DOPENCV_BIN_INSTALL_PATH=bin '-DBUILD_LIST=core,imgproc' -DBUILD_SHARED_LIBS=ON `
    -DBUILD_TESTS=OFF -DBUILD_PERF_TESTS=OFF -DBUILD_EXAMPLES=OFF -DBUILD_opencv_apps=OFF `
    -DBUILD_JAVA=OFF -DBUILD_opencv_python2=OFF -DBUILD_opencv_python3=OFF `
    -DWITH_IPP=OFF -DWITH_OPENCL=OFF -DWITH_ITT=OFF -DWITH_FFMPEG=OFF -DWITH_MSMF=OFF -DWITH_TBB=OFF
if ($LASTEXITCODE) { throw 'OpenCV CMake configuration failed.' }
& cmake --build $build --parallel 8
if ($LASTEXITCODE) { throw 'OpenCV build failed.' }
& cmake --install $build
if ($LASTEXITCODE) { throw 'OpenCV installation failed.' }
foreach ($dll in @('opencv_core4120.dll', 'opencv_imgproc4120.dll')) {
    Copy-Item -LiteralPath (Join-Path $install "bin/$dll") -Destination $runtime -Force
}
# The application uses the dynamically linked MSVC runtime, distributed beside its executable.
$crt = Get-ChildItem (Join-Path $vs 'VC/Redist/MSVC') -Directory |
    Where-Object { $_.Name -match '^\d+\.\d+\.\d+(\.\d+)?$' } |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (!$crt) { throw 'Cannot find a versioned MSVC redistributable directory.' }
$crtDir = Get-ChildItem (Join-Path $crt.FullName 'x64') -Directory |
    Where-Object { $_.Name -match '^Microsoft\.VC\d+\.CRT$' } |
    Select-Object -First 1 -ExpandProperty FullName
if (!$crtDir) { throw 'Cannot find the redistributable x64 MSVC CRT directory.' }
Copy-Item -Path (Join-Path $crtDir '*.dll') -Destination $runtime -Force
Copy-Item -LiteralPath (Join-Path $source 'LICENSE') -Destination (Join-Path $runtime 'OpenCV-LICENSE.txt') -Force
Write-Host "OpenCV $version ready: $install"
