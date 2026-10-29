---
name: 8sync-cli
description: Use this skill in EVERY session inside a repo whose AGENTS.md mentions 8sync. It teaches the AI which 8sync verbs (shot/pdf-img/locate/note/skill/run/review/new) to use instead of raw shell equivalents and how to keep the agents/* project memory (STATE/KNOWLEDGE) current. The AI MUST prefer the listed 8sync verbs over shell equivalents when one exists.
---

# 8sync-cli — bạn (AI) ĐANG chạy bên trong 8sync harness

**LOAD: bắt buộc cho mọi project có `AGENTS.md` đề cập 8sync.**

Bạn chạy dưới `omp` (oh-my-pi) được wrap bởi `8sync`. Dùng đúng các tool 8sync sẽ tiết kiệm token và giữ memory project nhất quán.

---

## 1. Quy tắc tuyệt đối

1. **LUÔN đọc `~/.omp/skills/karpathy-guidelines/SKILL.md` đầu tiên** trước mọi non-trivial task.
2. **LUÔN đọc `~/.omp/skills/image-routing/SKILL.md`** trước khi fetch/đọc bất kỳ tài nguyên hình ảnh, PDF, hoặc diff lớn.
3. **LUÔN đọc `agents/STATE.md` trước** (live plan), rồi các file `agents/*.md` liên quan — chúng chứa memory tích lũy của các session trước.
4. **Đọc project-local skills** trong `<repo>/.omp/skills/<name>/` khi task chạm vào lĩnh vực tương ứng — AGENTS.md đã liệt kê chúng giữa cặp sentinel `<!-- 8sync:skills:* -->`.
5. **CẬP NHẬT memory theo `~/.omp/agent/APPEND_SYSTEM.md`**: rewrite `agents/STATE.md` (Goal·Checklist·Current·Next) ở mọi phase boundary và trước khi context auto-compact; ghi learnings vào `agents/KNOWLEDGE.md` (`validated:` / `failure:`); procedure đã `validated:` → `agents/PLAYBOOKS.md`. Các file khác (`PROJECT.md`, `DECISIONS.md`, `PREFERENCES.md`) chỉ append khi có fact/quyết định/preference mới thật sự; `NOTES.md` ghi qua `8sync note`.

---

## 2. CLI tool 8sync bạn ĐƯỢC PHÉP dùng

Ưu tiên các lệnh này hơn equivalent của shell:

| Tình huống | Dùng | Thay vì |
|---|---|---|
| Cần screenshot UI/web route | `8sync shot <url\|file> [-o out.png]` (cần Chromium; không có → omp `browser` eval: `browser.open(url)` → `tab.screenshot`) | mô tả layout bằng text |
| Cần tọa độ/bbox phần tử trong ảnh | `8sync locate <img> "<prompt>"` (cần `8sync locate --setup` một lần) | đoán tọa độ |
| Cần đọc diff lớn (>300 dòng) | đọc text theo từng file/slice (`git diff --stat` rồi `git diff -- <file>`) — `8sync diff-img` hiện là **no-op stub**, KHÔNG dùng | dump cả `git diff` |
| Cần đọc PDF | `8sync pdf-img <file> [--page N]` | OCR/parse text |
| Tìm code/symbol | codegraph (skill `codegraph`) rồi tool `grep`/`glob` của omp — `8sync find` mở fzf + editor (interactive, treo agent); chỉ dùng `8sync find --no-open <kw>` nếu thật cần | `rg`/`fd` thô qua shell |
| Ghi nhớ ý tưởng/note thoáng qua | `8sync note [-t tag] "..."` | sửa `agents/NOTES.md` tay |
| Cài skill mới từ GitHub | `8sync skill add <url>` | clone tay |
| Commit + push + PR | Repo GitHub: `8sync ship "msg"` (`git add -A` TẤT CẢ → commit → push → `gh pr create`; `--no-pr`, `--draft`). Repo GitLab (vd vtiger): KHÔNG dùng `8sync ship` — theo project skill `commit` + `glab mr create` (skill `gitlab-mr` nếu có) | `git add && commit && push` từng bước tự chế |
| AI review PR/MR | `8sync review <n>` (in kết quả; `--post` mới đăng, GitHub qua `gh`, GitLab qua `glab`) | review tay |
| Khởi session mới / mở lại session đã đặt tên | `8sync new <name>` / `8sync . <name>` (`8sync .` = resume session gần nhất) | `cd` rồi mở omp mới |
| Chạy dev/build/test theo recipe | `8sync run dev\|build\|test\|fmt\|lint` (Cargo/package.json; không có thì chạy như shell command) | nhớ `pnpm dev`/`bun dev`/`cargo run` |

Khi không chắc lệnh nào, gọi `8sync help` hoặc `8sync flow`; mọi verb đều có `-h`.

---

## 3. Cấu trúc memory dự án

```
<repo>/
├── AGENTS.md                       ← bạn đọc đầu, link sang dưới + liệt kê skills
├── .omp/
│   └── skills/                     project-local skills (chỉ khi thêm rõ ràng qua `8sync skill add <url>`)
│       └── <name>/                 đọc SKILL.md / CLAUDE.md / README.md bên trong
└── agents/                         ← memory dự án (8sync seed khi thiếu)
    ├── STATE.md                    live plan: Goal·Checklist·Current·Next — rewrite mỗi phase boundary
    ├── KNOWLEDGE.md                append-only: `validated:` / `failure:` learnings
    ├── PLAYBOOKS.md                procedure đã validated, index theo `When:`
    ├── PROJECT.md                  facts cố định (stack, entrypoint)
    ├── DECISIONS.md                append-only: quyết định kiến trúc
    ├── PREFERENCES.md              append-only: style user thích
    ├── NOTES.md                    quick notes via `8sync note`
    ├── planning/                   cây planning của `/feature` (GSD)
    └── archive/                    STATE/notes cũ đã archive
```

**Đọc tại session start** (STATE trước). Trích dẫn khi áp dụng (vd "Theo `agents/DECISIONS.md:42` đã chốt dùng zustand").

---

## 4. Session memory

Hai lớp, dùng cả hai:
- **omp Mnemopi**: `recall`/`reflect` trước khi trả lời về session/quyết định cũ; `retain` fact bền (quyết định, convention, preference) sau khi xong.
- **`agents/*.md`**: STATE/KNOWLEDGE/PLAYBOOKS theo quy tắc §1.5 — đây là handoff đọc được bởi người và session sau.

---

## 5. Token discipline

- File code > 800 dòng → dùng outline (`codegraph node <file> --symbols-only`, serena symbols overview) thay vì đọc cả file.
- Tool output dài → tóm tắt vào nháp trước, rồi mới đọc slice cụ thể.
- Đừng dump `git log`/`ls -la` toàn bộ; gọi với filter cụ thể.

---

## 6. Cite convention

- Code reference: `path/to/file.rs:23-58` hoặc `file.rs:23` (single line).
- KHÔNG dùng "file lines 23-58" hoặc "in file.rs around 23".

---

## 7. Khi bạn KHÔNG biết phải làm gì

1. Đọc lại karpathy-guidelines.
2. Đọc `agents/STATE.md` + `agents/KNOWLEDGE.md`.
3. Gõ `8sync help` (qua shell tool) xem các lệnh.
4. Tra repo/tool trước; chỉ hỏi user khi thật sự bị chặn — ngắn, cụ thể, không lan man.
