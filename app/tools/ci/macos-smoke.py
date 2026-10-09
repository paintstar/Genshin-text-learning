"""启动本次构建的应用，数据写入 runner 临时目录，结束时只停止自己的进程。"""
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile

bundle = Path('app/target/release/bundle/macos/GenshinLangLearning.app').resolve()
with (bundle / 'Contents/Info.plist').open('rb') as source:
    info = plistlib.load(source)
resources = bundle / 'Contents/Resources/resources'
if not (resources / 'dict.db').is_file() or not (resources / 'story.gllpack').is_file():
    raise SystemExit('macOS 安装包缺少完整离线资源')
with tempfile.TemporaryDirectory(prefix='gll-离线 ') as temporary:
    environment = {**os.environ, 'GLL_DATA_DIR': temporary, 'HTTPS_PROXY': 'http://127.0.0.1:9', 'HTTP_PROXY': 'http://127.0.0.1:9'}
    process = subprocess.Popen([str(bundle / 'Contents/MacOS' / info['CFBundleExecutable'])], env=environment)
    try:
        subprocess.run([sys.executable, 'app/tools/ci/check-installed.py', temporary, str(resources / 'story.gllpack')], check=True)
        if process.poll() is not None:
            raise SystemExit('macOS 应用启动后提前退出')
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=20)
