#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
WeChat Official Account Draft Publishing Script
"""

import os
import sys
import json
import requests

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except Exception:
        pass

def publish_draft(appid, secret, title, cover_path, preview_img_path):
    print("[1/4] Fetching access_token...")
    token_url = "https://api.weixin.qq.com/cgi-bin/token"
    token_res = requests.get(token_url, params={
        "grant_type": "client_credential",
        "appid": appid,
        "secret": secret
    }).json()

    if "access_token" not in token_res:
        print(f"Error fetching token: {json.dumps(token_res, ensure_ascii=False)}")
        return False

    access_token = token_res["access_token"]
    print("Token fetched successfully.")

    print("[2/4] Uploading cover image material...")
    media_url = f"https://api.weixin.qq.com/cgi-bin/material/add_material?access_token={access_token}&type=image"
    with open(cover_path, "rb") as f:
        media_res = requests.post(media_url, files={"media": f}).json()

    if "media_id" not in media_res:
        print(f"Error uploading cover: {json.dumps(media_res, ensure_ascii=False)}")
        return False

    thumb_media_id = media_res["media_id"]
    print(f"Cover material uploaded. media_id: {thumb_media_id}")

    print("[3/4] Uploading content preview image...")
    upload_img_url = f"https://api.weixin.qq.com/cgi-bin/media/uploadimg?access_token={access_token}"
    content_img_url = ""
    if os.path.exists(preview_img_path):
        with open(preview_img_path, "rb") as f:
            img_res = requests.post(upload_img_url, files={"media": f}).json()
            content_img_url = img_res.get("url", "")
            print(f"Content image uploaded: {content_img_url}")

    print("[4/4] Submitting article to WeChat Drafts...")
    
    html_content = f"""
    <section style="font-size: 15px; color: #333333; line-height: 1.8; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <blockquote style="background: #f1f5f9; border-left: 4px solid #4d6bfe; padding: 12px 16px; margin: 16px 0; color: #475569; font-size: 14px; border-radius: 4px;">
        <strong>导读</strong>：DeepSeek 刚刚开源了面向未来的 Agent 框架 <strong>DeepSeek Harness</strong>（“一切皆插件”）。为了让大家不用每次都在终端敲命令行、开着黑框浏览器，我基于 <strong>tw93/Pake</strong> 和 Rust 手搓了一个官方风格的极轻桌面客户端 <strong>DSH-UI</strong>。体积仅 8.7MB，双击秒开，现已全部开源并提供 Mac 与 Windows 预编译安装包！
      </blockquote>

      <h2 style="font-size: 18px; font-weight: bold; color: #0f172a; border-bottom: 2px solid #4d6bfe; padding-bottom: 6px; margin-top: 28px; margin-bottom: 14px;">💡 为什么想做这个？</h2>
      <p style="margin-bottom: 12px;">前几天，DeepSeek 开源了备受瞩目的开发者预览版 <strong>DeepSeek Harness</strong>。</p>
      <p style="margin-bottom: 12px;">不得不说，这套“一切皆插件”的 Agent 架构非常惊艳，无论模型、工具、沙箱、调度还是记忆都可以随意组装。</p>
      <p style="margin-bottom: 12px;">但目前官方默认是通过命令行 <code>npx @deepseek-ai/dsh web</code> 启动一个本地 Web 服务，然后在浏览器里访问。很多朋友在使用时遇到了一些小痛点：</p>
      <ul style="padding-left: 20px; margin-bottom: 16px; color: #475569;">
        <li>每次想用都要打开终端手动敲命令；</li>
        <li>命令行窗口不能关，一关后台服务就挂了；</li>
        <li>浏览器标签页混在一起，容易误关；</li>
        <li>传统用 Electron 打包桌面端，体积动辄 150MB~300MB，非常吃内存。</li>
      </ul>
      <p style="margin-bottom: 16px;">于是，我基于推友 <strong>@tw93</strong> 广受好评的轻量框架 <strong>Pake</strong> 以及 <strong>Rust + Tauri</strong>，深度定制了一款<strong>原生极轻桌面客户端 —— DSH-UI</strong>。</p>

      <h2 style="font-size: 18px; font-weight: bold; color: #0f172a; border-bottom: 2px solid #4d6bfe; padding-bottom: 6px; margin-top: 28px; margin-bottom: 14px;">🌟 DSH-UI 有哪些核心优势？</h2>

      <p style="font-weight: bold; color: #1e293b; margin-top: 14px; margin-bottom: 6px;">1. 🍃 极致小巧：仅 8.7MB（Electron 的 1/20）</p>
      <p style="margin-bottom: 12px;">告别 Chromium 和臃肿的 Node 运行库。DSH-UI 直接调用系统原生的 WebView2 / WebKit 渲染容器，整个客户端安装包<strong>仅 8.7MB</strong>，运行内存占用减少 80% 以上，轻快如飞！</p>

      <p style="font-weight: bold; color: #1e293b; margin-top: 14px; margin-bottom: 6px;">2. ⚡ 双击即用：后台全生命周期静默管控</p>
      <p style="margin-bottom: 12px;">完全不需要打开终端：双击桌面图标，内置的 Rust 内核自动在后台<strong>静默拉起智能体服务</strong>；就绪后瞬间切入对话窗口；退出时自动彻底释放后台进程，不留任何幽灵进程！</p>

      <p style="font-weight: bold; color: #1e293b; margin-top: 14px; margin-bottom: 6px;">3. 🍎 苹果 Mac &amp; 🪟 Windows 双端原生支持</p>
      <ul style="padding-left: 20px; margin-bottom: 16px; color: #475569;">
        <li><strong>Mac 端</strong>：完美原生适配 Apple Silicon（M1/M2/M3/M4）以及 Intel 芯片，提供开箱即用的 <code>.dmg</code> 镜像；</li>
        <li><strong>Windows 端</strong>：支持 Windows 10 / 11 64位，提供 <code>.exe</code> 与 <code>.msi</code> 安装包。</li>
      </ul>

      <h2 style="font-size: 18px; font-weight: bold; color: #0f172a; border-bottom: 2px solid #4d6bfe; padding-bottom: 6px; margin-top: 28px; margin-bottom: 14px;">📸 客户端实机界面展示</h2>
      <p style="text-align: center; margin: 18px 0;">
        <img src="{content_img_url}" style="width: 100%; max-width: 600px; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.15);" />
      </p>

      <h2 style="font-size: 18px; font-weight: bold; color: #0f172a; border-bottom: 2px solid #4d6bfe; padding-bottom: 6px; margin-top: 28px; margin-bottom: 14px;">📥 如何下载使用？</h2>
      <p style="margin-bottom: 12px;">项目现已发布 <strong>v0.1.2</strong> 版本，大家可以直接根据自己的电脑系统下载安装包体验：</p>
      <div style="background: #f8fafc; border: 1px solid #e2e8f0; border-radius: 8px; padding: 14px; margin-bottom: 18px;">
        <p style="margin-bottom: 8px;">🍏 <strong>Mac 用户</strong>：下载 <code>DeepSeek.Harness_0.1.0_aarch64.dmg</code></p>
        <p style="margin-bottom: 8px;">🪟 <strong>Windows 用户</strong>：下载 <code>DeepSeek.Harness_0.1.0_x64-setup.exe</code></p>
        <p style="margin-bottom: 4px; color: #4d6bfe;">👉 Release 下载页：https://github.com/xtxo/dsh-ui/releases/tag/v0.1.2</p>
        <p style="color: #4d6bfe;">👉 在线官网主页：https://xtxo.github.io/dsh-ui/</p>
      </div>

      <h2 style="font-size: 18px; font-weight: bold; color: #0f172a; border-bottom: 2px solid #4d6bfe; padding-bottom: 6px; margin-top: 28px; margin-bottom: 14px;">🤝 致谢开源生态</h2>
      <p style="margin-bottom: 8px;">特别致敬与鸣谢以下优秀的开源项目：</p>
      <ul style="padding-left: 20px; margin-bottom: 16px; color: #475569;">
        <li><strong>tw93/Pake</strong>：极其优雅的 Rust Web 桌面化开发框架；</li>
        <li><strong>deepseek-ai/deepseek-harness</strong>：DeepSeek 开源的“一切皆插件” Agent 引擎。</li>
      </ul>

      <p style="text-align: center; margin-top: 30px; font-weight: bold; color: #4d6bfe;">
        如果对你有帮助，欢迎在 GitHub 给项目点个 ⭐ Star 支持！<br>
        GitHub 仓库：https://github.com/xtxo/dsh-ui
      </p>
    </section>
    """

    draft_url = f"https://api.weixin.qq.com/cgi-bin/draft/add?access_token={access_token}"
    draft_data = {
        "articles": [
            {
                "title": title,
                "author": "xtxo",
                "digest": "DeepSeek Harness 极轻桌面客户端开源：仅8.7MB，支持 Mac 与 Windows 双击秒开。",
                "content": html_content,
                "content_source_url": "https://github.com/xtxo/dsh-ui",
                "thumb_media_id": thumb_media_id,
                "need_open_comment": 1,
                "only_fans_can_comment": 0
            }
        ]
    }

    draft_json_bytes = json.dumps(draft_data, ensure_ascii=False).encode("utf-8")
    draft_res = requests.post(
        draft_url,
        data=draft_json_bytes,
        headers={"Content-Type": "application/json; charset=utf-8"}
    ).json()
    if "media_id" in draft_res:
        print(f"SUCCESS: Draft added! media_id: {draft_res['media_id']}")
        return True
    else:
        print(f"FAILED: {json.dumps(draft_res, ensure_ascii=False)}")
        return False

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python publish_to_wechat.py <WECHAT_APPID> <WECHAT_SECRET>")
        sys.exit(1)

    appid = sys.argv[1]
    secret = sys.argv[2]
    title = "仅8.7MB！我把DeepSeek Harness做成了桌面版"
    
    script_dir = os.path.dirname(os.path.abspath(__file__))
    root_dir = os.path.dirname(script_dir)
    cover_path = os.path.join(root_dir, "assets", "cover.jpg")
    preview_path = os.path.join(root_dir, "assets", "preview.png")

    publish_draft(appid, secret, title, cover_path, preview_path)
