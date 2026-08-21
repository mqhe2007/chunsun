#Requires -Version 5.1
# 注意：本脚本经 `irm ... | iex` 执行，含中文文案，服务端必须以
# `Content-Type: text/plain; charset=utf-8` 返回本文件（nginx 已对 /cli/*.ps1 配置），
# 否则 PowerShell 5.1 会按 ISO-8859-1 解码导致乱码。
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$BaseUrl    = "__CHUNSUN_CLI_DOWNLOAD_URL__"
$InstallDir = Join-Path $HOME ".local\bin"
$CommandName = "chunsun.exe"
$Target     = Join-Path $InstallDir $CommandName

Write-Host "[install] 开始安装 chunsun CLI..."

# 检测 CPU 架构（仅支持 x64）
$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne "AMD64") {
    Write-Error "[install] 错误: 不支持的 CPU 架构: $arch（当前仅支持 x64）"
    exit 1
}

$BinaryName = "chunsun-cli-windows-x64.exe"
$BinaryUrl  = "$BaseUrl/$BinaryName"
Write-Host "[install] 平台: windows-x64"

# 确保安装目录存在
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# 下载二进制文件
Write-Host "[install] 正在下载: $BinaryUrl"
try {
    Invoke-WebRequest -Uri $BinaryUrl -OutFile $Target -UseBasicParsing
} catch {
    Write-Error "[install] 下载失败: $_"
    exit 1
}

Write-Host "[install] 安装完成: $Target"

# 检查并追加用户级 PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathEntries = $userPath -split ";"

if ($pathEntries -notcontains $InstallDir) {
    $newPath = ($pathEntries + $InstallDir) -join ";"
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    # 同步当前会话
    $env:Path = "$InstallDir;$env:Path"
    Write-Host "[install] 已将 $InstallDir 加入用户 PATH（重启终端后对所有会话生效）"
}

# 验证安装
Write-Host "[install] 验证安装："
try {
    & $Target --version
} catch {
    Write-Host "[install] 安装完成，请重启终端后执行: chunsun --version"
}
