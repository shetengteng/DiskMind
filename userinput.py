#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
交互式用户输入脚本 - 短超时轮询版本

设计动机:
- Cursor Agent 的 `block_until_ms` 即使配很大,IDE 端在 ~150-180s
  没有有效输出时也会触发 "interrupted by the user" 中断
- 让脚本主动在 IDE 中断之前**正常退出**,让 Agent 知道是"无事件超时"
  而不是"用户取消",可以无脑重新跑脚本继续等
- 退出码区分:
    0 → 用户输入了新内容(stdout 输出新内容)
    2 → 无事件超时(stdout 输出 ⏰ 超时提示),Agent 应**立即重跑本脚本**
    其他 → 程序错误

Agent 处理规范(配合 .cursor/rules/userinput.mdc):
    while True:
        ec = run("python3 userinput.py")
        if ec == 0:
            根据 prompts.txt 内容决定下一步
        elif ec == 2:
            重跑本脚本,继续等
        else:
            停止
"""

import os
import sys
import time

POLL_INTERVAL = 2
# 150s 是经验值:Cursor 默认 block_until_ms 是 30s 必中断;调大到 150s
# 仍然在 IDE 客户端的"无 stdout 输出最长容忍时间"内,既给了用户两分钟
# 思考时间,也不会被外部强制 kill。Agent 收到正常 exit code 2 后可以
# 无成本重跑(下一次又是新的 150s 窗口)。
SOFT_TIMEOUT_SECS = 150

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROMPTS_FILE = os.path.join(SCRIPT_DIR, 'prompts.txt')


def read_prompts():
    """返回 (stripped_content, ok)。不存在 / 空 / 读失败 都返回 ('', False)。"""
    if not os.path.exists(PROMPTS_FILE):
        return '', False
    try:
        with open(PROMPTS_FILE, 'r', encoding='utf-8') as f:
            content = f.read().strip()
        if not content:
            return '', False
        return content, True
    except Exception as e:
        print(f"读取 prompts.txt 出错: {e}", file=sys.stderr)
        return '', False


def main():
    initial_content, _ = read_prompts()

    print(f"⏳ 等待 prompts.txt 内容变化(超时 {SOFT_TIMEOUT_SECS}s,每 {POLL_INTERVAL}s 检查一次)")
    if initial_content:
        preview = initial_content[:80] + ('...' if len(initial_content) > 80 else '')
        print(f"    当前内容: {preview}")
    else:
        print("    当前文件为空或不存在。")
    print("    请修改 prompts.txt 的内容,脚本会自动检测变化。")
    sys.stdout.flush()

    elapsed = 0
    while elapsed < SOFT_TIMEOUT_SECS:
        time.sleep(POLL_INTERVAL)
        elapsed += POLL_INTERVAL

        content, ok = read_prompts()
        if ok and content != initial_content:
            print(f"\n✅ 检测到内容变化(等待了 {elapsed}s),新内容如下:")
            print("=" * 60)
            print(content)
            print("=" * 60)
            sys.stdout.flush()
            sys.exit(0)

    print(f"\n⏰ 软超时({SOFT_TIMEOUT_SECS}s 内 prompts.txt 未更新)。"
          f"Agent 应立即重新运行本脚本继续等待。")
    sys.stdout.flush()
    sys.exit(2)


if __name__ == "__main__":
    main()
