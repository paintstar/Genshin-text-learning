$ErrorActionPreference = 'Stop'
$installer = Get-ChildItem 'app/target/release/bundle/nsis/*-setup.exe' | Select-Object -First 1
if (!$installer) { throw '未生成 Windows 安装包' }
$installDir = Join-Path $env:RUNNER_TEMP '提瓦特 离线安装'
$dataDir = Join-Path $env:RUNNER_TEMP '提瓦特 学习数据'
New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
$setup = Start-Process -FilePath $installer.FullName -ArgumentList "/S /D=$installDir" -Wait -PassThru
if ($setup.ExitCode -ne 0) { throw 'Windows 静默安装失败' }
$config = Get-Content 'app/crates/app/tauri.conf.json' -Raw | ConvertFrom-Json
$binary = Get-Item (Join-Path $installDir ($config.mainBinaryName + '.exe'))
if (!(Test-Path (Join-Path $installDir 'resources/dict.db')) -or !(Test-Path (Join-Path $installDir 'resources/story.gllpack'))) { throw '安装包缺少完整离线资源' }
$ruleName = "GLL-offline-$env:GITHUB_RUN_ID"
$appProcess = $null
try {
  # 仅阻止本次安装的应用访问外网，不影响 runner 和其他进程。
  New-NetFirewallRule -DisplayName $ruleName -Direction Outbound -Program $binary.FullName -Action Block | Out-Null
  $env:GLL_DATA_DIR = $dataDir
  $appProcess = Start-Process -FilePath $binary.FullName -PassThru
  python app/tools/ci/check-installed.py $dataDir (Join-Path $installDir 'resources/story.gllpack')
  if ($LASTEXITCODE -ne 0) { throw 'Windows 断网启动失败' }
  $appProcess.Refresh()
  if ($appProcess.HasExited) { throw 'Windows 应用启动后提前退出' }
  if ($appProcess.MainWindowHandle -eq 0) { throw 'Windows 应用没有显示主窗口' }
  Add-Type @'
using System;
using System.Runtime.InteropServices;
public class GllSmokeWindow {
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags);
}
'@
  [GllSmokeWindow]::SetWindowPos($appProcess.MainWindowHandle, [IntPtr]::Zero, 0, 0, 960, 640, 0x0040) | Out-Null
  Start-Sleep -Milliseconds 1000
  Add-Type -AssemblyName System.Windows.Forms
  Add-Type -AssemblyName System.Drawing
  $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
  New-Item -ItemType Directory 'app/target/smoke' -Force | Out-Null
  $bitmap.Save((Join-Path (Resolve-Path 'app/target/smoke') 'windows.png'), [System.Drawing.Imaging.ImageFormat]::Png)
  $graphics.Dispose()
  $bitmap.Dispose()
} finally {
  if ($appProcess -and !$appProcess.HasExited) { Stop-Process -Id $appProcess.Id }
  Remove-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue
  Remove-Item Env:GLL_DATA_DIR -ErrorAction SilentlyContinue
}
