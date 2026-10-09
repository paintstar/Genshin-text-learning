"""只检查本次 CI 隔离数据目录，等待随包资源导入完成。"""
import gzip
import json
from pathlib import Path
import sqlite3
import sys
import time

data_dir, pack_path = map(Path, sys.argv[1:3])
with gzip.open(pack_path, 'rt', encoding='utf-8') as source:
    expected = json.loads(source.readline())
deadline = time.monotonic() + 180
while time.monotonic() < deadline:
    try:
        with sqlite3.connect(data_dir / 'app.db', timeout=2) as db:
            row = db.execute("SELECT value FROM settings_kv WHERE key='resources.story_info'").fetchone()
            info = json.loads(row[0]) if row else None
            if info and info['dataVersion'] == expected['dataVersion'] and info['questCount'] == expected['questCount']:
                assert db.execute('PRAGMA quick_check').fetchone()[0] == 'ok'
                print(f"启动成功：已离线导入 {info['questCount']} 个双语任务")
                break
    except (sqlite3.Error, json.JSONDecodeError):
        pass
    time.sleep(1)
else:
    raise SystemExit('应用未在限定时间内完成随包剧情导入')
