//! The task as a story for the task page: which phase each run worked on, what it
//! did (summary, digest, commits, why it failed), which run each question and note
//! belongs to, what is happening now, what still waits on the boss, and every
//! decision taken (by the boss, the Leader/PO, the coordinator for the boss, or a
//! re-graded AC). Read-only over task.json, the run logs' cached digests, the PM
//! feed, the planning tree and git.
use super::*;

/// A commit on the task branch.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(super) struct Commit {
    pub sha: String,
    pub subject: String,
    #[serde(skip)]
    pub at: i64,
}

fn secs(iso: &str) -> Option<i64> {
    pm::iso_secs(iso)
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// `[started, ended]` of a run in unix seconds (a running run ends now).
fn window(r: &Run) -> (i64, i64) {
    let s = secs(&r.started).unwrap_or(0);
    (s, r.ended.as_deref().and_then(secs).unwrap_or_else(now_secs))
}

/// Commits the task's runs made: `base..branch`, or after the merge (that range is
/// then empty) the branch since the task was created; only commits made inside one
/// of the task's runs (a base branch moving meanwhile is someone else's work).
pub(super) fn task_commits(root: &FsPath, t: &Task) -> Vec<Commit> {
    let Some(br) = &t.branch else { return Vec::new() };
    let log = |range: &[&str]| -> Vec<Commit> {
        let mut args = vec!["log", "--no-merges", "--format=%h%x09%ct%x09%s"];
        args.extend_from_slice(range);
        git(root, &args)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let mut p = l.splitn(3, '\t');
                Some(Commit { sha: p.next()?.to_string(), at: p.next()?.parse().ok()?, subject: p.next().unwrap_or("").to_string() })
            })
            .collect()
    };
    let mut commits = match &t.base {
        Some(base) => log(&[&format!("{base}..{br}")]),
        None => Vec::new(),
    };
    if commits.is_empty() {
        let since = format!("--since={}", t.created);
        commits = log(&[&since, br.as_str()]);
    }
    let windows: Vec<(i64, i64)> = t.runs.iter().map(window).collect();
    commits.retain(|c| windows.iter().any(|(s, e)| c.at >= *s && c.at <= e + 60));
    commits
}

/// Planning steps that work on one `/feature` phase.
fn phase_step(s: Stage) -> bool {
    matches!(s, Stage::Solution | Stage::Review | Stage::Dev | Stage::Test | Stage::Security)
}

/// Phase each run worked on: recorded when the run started; for older runs, the
/// first PM status tick inside the run (`Dev M1 đang dev …`, written while it ran,
/// so a ship run that moved STATE to the next phase still reads as its own); a run
/// with no tick (shorter than the 5-minute tick) takes its neighbour's: a plan
/// from the run after it, any other step from the run before it. Intake, report
/// and tasks without phases: none.
pub(super) fn run_phases(root: &FsPath, t: &Task) -> Vec<Option<String>> {
    if feature_dir(root, t).is_none() {
        return vec![None; t.runs.len()];
    }
    let ticks: Vec<(i64, Option<String>)> = pm::task_ticks(root, &t.id).into_iter().filter_map(|(at, text)| Some((secs(&at)?, pm::tick_phase(&text)))).collect();
    let mut out: Vec<Option<String>> = t
        .runs
        .iter()
        .map(|r| {
            if !phase_step(r.stage) {
                return None;
            }
            r.phase.clone().or_else(|| {
                let (s, e) = window(r);
                ticks.iter().find(|(at, _)| *at >= s && *at <= e).and_then(|(_, p)| p.clone())
            })
        })
        .collect();
    for i in 0..out.len() {
        if out[i].is_some() || !phase_step(t.runs[i].stage) {
            continue;
        }
        let pick = if t.runs[i].stage == Stage::Solution {
            (i + 1..out.len()).find_map(|j| out[j].clone())
        } else {
            (0..i).rev().find_map(|j| out[j].clone())
        };
        out[i] = pick;
    }
    out
}

/// Index of the run a timestamped event belongs to: started strictly before it and
/// not ended more than a few seconds before it (a run's result is posted as it closes).
fn run_at(t: &Task, at: &str) -> Option<usize> {
    let a = secs(at)?;
    t.runs.iter().rposition(|r| {
        let (s, e) = window(r);
        s < a && a <= e + 5
    })
}

/// `results/<run>.json` message, if the run wrote one.
fn result_message(dir: &FsPath, log: &str) -> Option<String> {
    let raw = std::fs::read_to_string(result_file(dir, log)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v.get("message").and_then(|m| m.as_str()).map(str::trim).filter(|m| !m.is_empty()).map(str::to_string)
}

/// Plain one-liners of a markdown text: bullets/headings/bold unwrapped, rules skipped.
fn plain_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().trim_start_matches('#').trim().trim_start_matches(['-', '*', '•']).trim())
        .filter(|l| !l.is_empty() && !l.starts_with('|') && !l.starts_with("```") && !l.chars().all(|c| matches!(c, '-' | '=' | '*' | '_')))
        .map(|l| l.replace("**", ""))
        .collect()
}

/// ≤ 3 summary lines: each line of the text; a single long paragraph split into sentences.
pub(super) fn summary_lines(text: &str) -> Vec<String> {
    let lines = plain_lines(text);
    let lines = if lines.len() == 1 && lines[0].chars().count() > 220 {
        lines[0].split_inclusive(". ").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else {
        lines
    };
    lines.into_iter().take(3).map(|l| clip(&l, 220)).collect()
}

/// First line of the `## Blocking` / `## Lỗi chặn` / `## FAIL` section of a review
/// or verification doc that names a finding.
fn first_finding(md: &str) -> Option<String> {
    let mut on = false;
    for l in md.lines() {
        let t = l.trim();
        if t.starts_with('#') {
            let h = t.to_lowercase();
            on = ["blocking", "chặn", "fail", "finding"].iter().any(|k| h.contains(k));
            continue;
        }
        if on {
            let p = plain_lines(t);
            if let Some(line) = p.first().filter(|x| !x.to_lowercase().starts_with("không có") && !x.eq_ignore_ascii_case("none")) {
                return Some(clip(line, 300));
            }
        }
    }
    None
}

/// Why a run failed: its result message, else the line of its closing words naming
/// the blocker, else (only for the latest run of its step, whose doc it wrote) the
/// first finding of REVIEW / TEST / VERIFICATION.
fn fail_reason(root: &FsPath, t: &Task, i: usize, message: Option<&str>, last_text: Option<&str>) -> Option<String> {
    let r = &t.runs[i];
    if r.verdict.as_deref() != Some("fail") {
        return None;
    }
    if let Some(m) = message.and_then(|m| plain_lines(m).into_iter().next()) {
        return Some(clip(&m, 300));
    }
    if let Some(text) = last_text {
        let lines = plain_lines(text);
        let keys = ["chặn", "block", "fail", "lỗi"];
        if let Some(l) = lines.iter().find(|l| keys.iter().any(|k| l.to_lowercase().contains(k))).or(lines.first()) {
            return Some(clip(l, 300));
        }
    }
    let latest = t.runs.iter().rposition(|x| x.stage == r.stage) == Some(i);
    if !latest {
        return None;
    }
    let dir = task_dir(root, &t.id);
    let doc = match r.stage {
        Stage::Review => std::fs::read_to_string(dir.join("REVIEW.md")).ok(),
        Stage::Test => std::fs::read_to_string(dir.join("TEST.md")).ok().or_else(|| {
            let ph = run_phases(root, t).get(i).cloned().flatten()?;
            let f = feature_dir(root, t)?;
            std::fs::read_dir(f.join("phases")).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n == ph || n.starts_with(&format!("{ph}-")))).and_then(|d| std::fs::read_to_string(d.join(format!("{ph}-VERIFICATION.md"))).ok())
        }),
        Stage::Security => std::fs::read_to_string(dir.join("SECURITY.md")).ok(),
        _ => None,
    }?;
    first_finding(&doc)
}

/// Digests of the task's finished runs (a running run has none yet): cached ones
/// read, missing ones streamed in parallel and cached.
fn run_facts(root: &FsPath, t: &Task) -> Vec<Option<digest::Facts>> {
    let dir = task_dir(root, &t.id);
    let mut out: Vec<Option<digest::Facts>> = t.runs.iter().map(|r| r.ended.as_ref().and_then(|_| digest::cached(&dir, &r.log))).collect();
    let missing: Vec<usize> = t.runs.iter().enumerate().filter(|(i, r)| r.ended.is_some() && out[*i].is_none() && dir.join("logs").join(&r.log).exists()).map(|(i, _)| i).collect();
    let (d, runs) = (&dir, &t.runs);
    let computed: Vec<(usize, digest::Facts)> = std::thread::scope(|s| {
        let handles: Vec<_> = missing.iter().map(|&i| (i, s.spawn(move || digest::of_log(d, &runs[i].log)))).collect();
        handles.into_iter().filter_map(|(i, h)| h.join().ok().map(|f| (i, f))).collect()
    });
    for (i, f) in computed {
        out[i] = Some(f);
    }
    out
}

/// A note posted into a running step (`user → dev`, `coordinator → test`).
fn is_note_into_step(from: &str) -> bool {
    from.starts_with("user → ") || from.starts_with("coordinator → ")
}

/// Thread entries the boss's buttons write (not free text needing an answer).
fn is_boss_action(text: &str) -> bool {
    ["Duyệt giải pháp.", "Nghiệm thu.", "Đóng.", "Tiếp tục."].contains(&text)
        || ["[Bắt đầu]", "[Tiếp tục]", "[Tạm dừng]", "Bạn duyệt kế hoạch", "Code tiếp theo kế hoạch"].iter().any(|p| text.starts_with(p))
}

fn is_gate(text: &str) -> bool {
    text == "Duyệt giải pháp." || text == "Nghiệm thu." || text.starts_with("Bạn duyệt kế hoạch") || text.starts_with("Code tiếp theo kế hoạch")
}

fn is_delegated(from: &str, text: &str) -> bool {
    from == "coordinator" && text.starts_with("Duyệt kế hoạch thay sếp")
}

/// `[Q-3] …` → Q-3.
fn ask_id(text: &str) -> Option<&str> {
    let id = text.strip_prefix('[')?.split(']').next()?;
    (id.starts_with("Q-") && id[2..].chars().all(|c| c.is_ascii_digit()) && id.len() > 2).then_some(id)
}

/// boss | leader | report | question | answer | coordinator | delegated_approval | system
fn thread_kind(t: &Task, m: &Msg, run: Option<usize>) -> &'static str {
    let from = m.from.as_str();
    if from == "user" || from.starts_with("user → ") {
        return "boss";
    }
    if is_delegated(from, &m.text) {
        return "delegated_approval";
    }
    if from == "coordinator" || from.starts_with("coordinator → ") {
        return "coordinator";
    }
    if from == "system" {
        return "system";
    }
    if from.contains(" → ") && ask_id(&m.text).is_some() {
        return "question";
    }
    if ask_id(&m.text).is_some() && !m.text.contains("] chuyển cho sếp:") {
        return "answer";
    }
    if run.is_some_and(|i| t.runs[i].stage == Stage::Report) {
        return "report";
    }
    "leader"
}

/// Notes the engine delivered (`ask/inbox/M-<n>.sent`): (text as posted in the thread).
fn delivered_notes(root: &FsPath, id: &str) -> Vec<String> {
    std::fs::read_dir(inbox::inbox_dir(root, id))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sent"))
        .filter_map(|p| serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(p).ok()?).ok())
        .map(|v| {
            let text = v["text"].as_str().unwrap_or("").trim().to_string();
            if v["kind"] == "pause" { format!("[Tạm dừng] {text}").trim().to_string() } else { text }
        })
        .collect()
}

fn same_note(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    let head = |s: &str| s.chars().take(80).collect::<String>();
    !a.is_empty() && (a == b || head(a) == head(b))
}

/// Everything the task page adds to the stored task.
pub(super) struct Story {
    pub now: Option<serde_json::Value>,
    pub unanswered: Vec<serde_json::Value>,
    pub report: Option<serde_json::Value>,
}

/// Enrich the serialized task in place (`runs[i]`: phase, summary, digest, commits,
/// fail_reason; `asks[i].run`; `thread[i]`: kind, run, ack, reply) and return the
/// page-level parts.
pub(super) fn enrich(root: &FsPath, t: &Task, task: &mut serde_json::Value) -> Story {
    let dir = task_dir(root, &t.id);
    let phases = run_phases(root, t);
    let facts = run_facts(root, t);
    let commits = task_commits(root, t);
    let log_of = |i: Option<usize>| i.map(|i| t.runs[i].log.clone());
    // The Leader's / Reviewer's thread message posted for a run (its result, as the
    // boss read it) is the preferred summary.
    let said: Vec<Option<&str>> = (0..t.runs.len())
        .map(|i| {
            t.thread
                .iter()
                .rev()
                .filter(|m| matches!(m.from.as_str(), "leader" | "reviewer") && ask_id(&m.text).is_none() && run_at(t, &m.at) == Some(i))
                .map(|m| m.text.as_str())
                .next()
        })
        .collect();
    if let Some(runs) = task["runs"].as_array_mut() {
        for (i, r) in runs.iter_mut().enumerate() {
            let run = &t.runs[i];
            let message = result_message(&dir, &run.log);
            let f = facts[i].as_ref();
            let (summary, source) = match (said[i], &message, f.and_then(|f| f.last_text.as_deref())) {
                (Some(s), _, _) => (summary_lines(s), Some("thread")),
                (None, Some(m), _) => (summary_lines(m), Some("result")),
                (None, None, Some(last)) => (summary_lines(last), Some("log")),
                _ => (Vec::new(), None),
            };
            r["phase"] = serde_json::json!(phases[i]);
            r["summary"] = serde_json::json!(summary);
            r["summary_source"] = serde_json::json!(source);
            r["digest"] = match f {
                Some(f) => {
                    let subs = usage::subagent_sessions(&dir, &dir.join("logs").join(&run.log), run);
                    let mut models: Vec<String> = Vec::new();
                    for (_, _, m, _) in &subs {
                        if !models.contains(m) {
                            models.push(m.clone());
                        }
                    }
                    serde_json::json!({
                        "tools": f.tools, "errors": f.errors, "harness_errors": f.harness_errors,
                        "files": f.files, "last_error": f.last_error,
                        "subagents": subs.len(), "subagent_models": models,
                        "subagents_list": subs.iter().map(|(id, _, m, e)| serde_json::json!({ "id": id, "name": id, "model": m, "effort": e })).collect::<Vec<_>>(),
                    })
                }
                None => serde_json::Value::Null,
            };
            let (s, e) = window(run);
            let mine: Vec<&Commit> = commits.iter().filter(|c| c.at >= s && c.at <= e + 60).collect();
            r["commits"] = serde_json::json!(mine);
            r["fail_reason"] = serde_json::json!(fail_reason(root, t, i, message.as_deref(), f.and_then(|f| f.last_text.as_deref())));
        }
    }
    if let Some(asks) = task["asks"].as_array_mut() {
        for (a, ask) in asks.iter_mut().zip(&t.asks) {
            a["run"] = serde_json::json!(log_of(run_at(t, &ask.at)));
        }
    }
    let sent = delivered_notes(root, &t.id);
    let mut unanswered = Vec::new();
    if let Some(thread) = task["thread"].as_array_mut() {
        for (k, (j, m)) in thread.iter_mut().zip(&t.thread).enumerate() {
            let run = run_at(t, &m.at);
            j["kind"] = serde_json::json!(thread_kind(t, m, run));
            j["run"] = serde_json::json!(log_of(run));
            let boss = m.from == "user" || m.from.starts_with("user → ");
            // Button actions ([Tạm dừng] with or without a note, [Tiếp tục], Duyệt…)
            // are not messages: they expect no reply.
            let action = boss && is_boss_action(&m.text);
            j["action"] = serde_json::json!(action);
            if is_note_into_step(&m.from) {
                // Acknowledged = the engine delivered it into the live session (it took
                // the inbox file, or the run's log shows the steer).
                let steer = run.and_then(|i| facts[i].as_ref()).and_then(|f| f.steers.iter().find(|s| same_note(&s.text, &m.text)));
                let ack = steer.is_some() || sent.iter().any(|s| same_note(s, &m.text));
                j["ack"] = serde_json::json!(ack);
                j["reply"] = serde_json::json!(steer.and_then(|s| s.reply.clone()));
                // Never delivered, once its run is over or a minute has passed: the
                // step never saw it, the boss is still waiting.
                let settled = run.is_none_or(|i| t.runs[i].ended.is_some()) || secs(&m.at).is_some_and(|a| now_secs() - a > 60);
                if boss && !action && !ack && settled {
                    unanswered.push(serde_json::json!({ "at": m.at, "text": m.text, "run": log_of(run) }));
                }
            } else {
                j["ack"] = serde_json::json!(false);
                if m.from == "user" && !action {
                    let answered = t.thread[k + 1..].iter().any(|n| !(n.from == "user" || n.from.starts_with("user → ") || n.from == "system" || n.from.starts_with("coordinator")));
                    if !answered {
                        unanswered.push(serde_json::json!({ "at": m.at, "text": m.text, "run": null }));
                    }
                }
            }
        }
    }
    let now = (t.status == Status::Running).then(|| t.runs.last()).flatten().map(|r| {
        let phase = r.phase.clone().or_else(|| phases.last().cloned().flatten());
        let recent = digest::tail_actions(&dir.join("logs").join(&r.log), 5);
        let mut files: Vec<String> = Vec::new();
        for p in recent.iter().filter_map(|a| a.file.as_deref()) {
            if !files.iter().any(|f| f == p) {
                files.push(p.to_string());
            }
        }
        let started = secs(&r.started).unwrap_or(0);
        let tick = pm::task_ticks(root, &t.id).into_iter().rev().find(|(at, _)| secs(at).is_some_and(|a| a >= started));
        // No PM tick yet (the first comes after 5 minutes): say it from the log tail.
        let (activity, at, source) = match (tick, recent.last()) {
            (Some((at, text)), _) => (Some(text), Some(at), Some("pm")),
            (None, Some(a)) => {
                let mins = (now_secs() - started).max(0) / 60;
                let ph = phase.as_deref().map(|p| format!(" {p}")).unwrap_or_default();
                // `đang chờ subagent: …` / `đang chạy bash` already say it; a plain intent gets `đang: tool: …`.
                let doing = match &a.tool {
                    _ if a.text.starts_with("đang ") => a.text.clone(),
                    Some(tool) => format!("đang: {tool}: {}", a.text),
                    None => format!("đang: {}", a.text),
                };
                (Some(format!("{} · {}{ph} · {mins} phút · {doing}", r.stage.who(), r.stage.label())), a.at.clone(), Some("log"))
            }
            // No PM tick and nothing readable in the log yet (not written, or empty):
            // still say the step runs.
            (None, None) => {
                let mins = (now_secs() - started).max(0) / 60;
                let ph = phase.as_deref().map(|p| format!(" {p}")).unwrap_or_default();
                (Some(format!("{} · {}{ph} · {mins} phút · đang chạy", r.stage.who(), r.stage.label())), Some(r.started.clone()), Some("run"))
            }
        };
        serde_json::json!({ "activity": activity, "at": at, "source": source, "recent": recent, "files": files, "stage": r.stage, "phase": phase, "started": r.started })
    });
    let report = std::fs::read_to_string(dir.join("REPORT.md")).ok().map(|md| serde_json::json!({ "decide": report_decide(&md) }));
    Story { now, unanswered, report }
}

/// Items of the report's "Sếp cần quyết" section (a heading, or a list item with
/// sub-items): one string per item.
pub(super) fn report_decide(md: &str) -> Vec<String> {
    let lines: Vec<&str> = md.lines().collect();
    let low = |s: &str| s.to_lowercase();
    let markers = ["sếp cần quyết", "cần sếp quyết", "boss cần quyết", "cần quyết định", "boss decides", "decisions needed"];
    let Some(start) = lines.iter().position(|l| markers.iter().any(|m| low(l).contains(m))) else { return Vec::new() };
    let indent = |s: &str| s.len() - s.trim_start().len();
    let bullet = |s: &str| -> Option<String> {
        let t = s.trim_start();
        let rest = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")).or_else(|| {
            let d = t.find(|c: char| !c.is_ascii_digit())?;
            (d > 0).then(|| t[d..].strip_prefix(". ").or_else(|| t[d..].strip_prefix(") "))).flatten()
        })?;
        Some(rest.trim_start_matches("[ ] ").trim().to_string())
    };
    let head = lines[start];
    let is_heading = head.trim_start().starts_with('#');
    let base = indent(head);
    let mut out: Vec<String> = Vec::new();
    // `**Sếp cần quyết:** X` on one line: X is the item.
    if let Some((_, after)) = head.split_once(':').filter(|_| !is_heading) {
        let after = after.trim().trim_start_matches("**").trim();
        if !after.is_empty() {
            out.push(after.replace("**", ""));
        }
    }
    let mut item_indent: Option<usize> = None;
    for l in &lines[start + 1..] {
        if l.trim().is_empty() {
            continue;
        }
        if l.trim_start().starts_with('#') {
            break;
        }
        if !is_heading && indent(l) <= base {
            break;
        }
        match bullet(l) {
            Some(b) if item_indent.is_none_or(|i| indent(l) <= i) => {
                item_indent = Some(indent(l));
                out.push(b.replace("**", ""));
            }
            // A deeper bullet or a wrapped line continues the item.
            _ => match out.last_mut() {
                Some(last) if item_indent.is_some() => {
                    last.push(' ');
                    last.push_str(bullet(l).unwrap_or_else(|| l.trim().to_string()).replace("**", "").as_str());
                }
                _ if is_heading => break,
                _ => {}
            },
        }
    }
    out
}

#[derive(Serialize, Debug, PartialEq)]
pub(super) struct Decision {
    pub at: Option<String>,
    /// boss | leader | reviewer | coordinator
    pub by: &'static str,
    pub phase: Option<String>,
    /// gate | delegated | answer | plan | ac_regrade
    pub kind: &'static str,
    pub text: String,
    /// Why (Key Decisions "Lý do"), or the question an answer settles.
    pub why: Option<String>,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    /// Run log the decision was taken in (answers).
    pub run: Option<String>,
    /// The table's "Ai quyết" cell (`PO (board_ask Q-2)`), to join answers to rows.
    #[serde(skip)]
    pub who: String,
}

/// Whether `s` names question `id` (`Q-2`, not `Q-21`).
fn names_ask(s: &str, id: &str) -> bool {
    s.match_indices(id).any(|(i, _)| !s[i + id.len()..].starts_with(|c: char| c.is_ascii_digit()))
}

/// Rows of the first table under a "Key Decisions" heading.
fn key_decisions(md: &str, file: &str) -> Vec<Decision> {
    let mut out = Vec::new();
    let mut in_section = false;
    let mut header: Vec<String> = Vec::new();
    let mut seen_rule = false;
    for l in md.lines() {
        let t = l.trim();
        if t.starts_with('#') {
            if in_section && seen_rule {
                break;
            }
            let h = t.to_lowercase();
            in_section = h.contains("key decision") || h.contains("quyết định chính");
            continue;
        }
        if !in_section || !t.starts_with('|') {
            if in_section && seen_rule && !t.is_empty() {
                break;
            }
            continue;
        }
        let cells: Vec<String> = t.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect();
        if cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))) {
            seen_rule = true;
            continue;
        }
        if !seen_rule {
            header = cells.iter().map(|c| c.to_lowercase()).collect();
            continue;
        }
        let col = |names: &[&str]| header.iter().position(|h| names.iter().any(|n| h.starts_with(n))).and_then(|i| cells.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != "—" && s != "-");
        let Some(text) = col(&["quyết định", "decision"]) else { continue };
        let who_raw = col(&["ai quyết", "who", "by", "người quyết"]).unwrap_or_default();
        let who = who_raw.to_lowercase();
        // The Leader writes and owns these rows (`đề xuất`, `auto`, `PO (board_ask Q-2)`,
        // `PO (REVIEW B1)`): leader. Only a row the boss settled himself (`user`, the
        // table's own convention) is the boss's.
        let by = if ["user", "boss", "sếp"].iter().any(|k| who.starts_with(k)) { "boss" } else { "leader" };
        out.push(Decision {
            at: col(&["ngày", "date", "khi"]),
            by,
            phase: col(&["phase"]),
            kind: "plan",
            text,
            why: col(&["lý do", "why", "rationale", "vì sao"]),
            reference: Some(file.to_string()),
            run: None,
            who: who_raw,
        });
    }
    out
}

/// Every decision of the task, oldest first (rows with only a date sort by it):
/// Key Decisions tables (PROJECT.md, phase CONTEXT), the boss's gate approvals,
/// the coordinator's approvals on the boss's behalf, answered questions, and ACs
/// graded with a note.
pub(super) fn decisions(root: &FsPath, t: &Task) -> Vec<Decision> {
    let mut out = Vec::new();
    let phases = run_phases(root, t);
    let phase_at = |at: &str| -> Option<String> {
        let a = secs(at)?;
        t.runs.iter().enumerate().rev().find(|(_, r)| secs(&r.started).is_some_and(|s| s <= a)).and_then(|(i, _)| phases[i].clone())
    };
    if let Some(f) = feature_dir(root, t) {
        if let Ok(md) = std::fs::read_to_string(f.join("PROJECT.md")) {
            out.extend(key_decisions(&md, "PROJECT.md"));
        }
        let mut ctx: Vec<PathBuf> = std::fs::read_dir(f.join("phases"))
            .into_iter()
            .flatten()
            .flatten()
            .flat_map(|d| std::fs::read_dir(d.path()).into_iter().flatten().flatten().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with("-CONTEXT.md")))
            .collect();
        ctx.sort();
        for p in ctx {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if let Ok(md) = std::fs::read_to_string(&p) {
                out.extend(key_decisions(&md, &name));
            }
        }
    }
    for m in &t.thread {
        let (by, kind) = if m.from == "user" && is_gate(&m.text) {
            ("boss", "gate")
        } else if is_delegated(&m.from, &m.text) {
            ("coordinator", "delegated")
        } else {
            continue;
        };
        let phase = if m.text == "Nghiệm thu." { None } else { phase_at(&m.at) };
        out.push(Decision { at: Some(m.at.clone()), by, phase, kind, text: m.text.clone(), why: None, reference: None, run: None, who: String::new() });
    }
    for a in t.asks.iter().filter(|a| a.status == "answered") {
        let Some(answer) = a.answer.as_deref() else { continue };
        let run = run_at(t, &a.at);
        let at = a.answered_at.clone().or_else(|| Some(a.at.clone()));
        if let (Some(ok), Some(c)) = (a.approved(), &a.ac) {
            let ph = a.phase.as_deref().map(|p| format!(" ({p})")).unwrap_or_default();
            let verb = if ok { "Duyệt" } else { "Không duyệt" };
            let proposal = if c.new.trim().is_empty() { String::new() } else { format!(": {}", c.new.trim()) };
            out.push(Decision {
                at,
                by: "boss",
                phase: a.phase.clone(),
                kind: "ac_change",
                text: format!("{verb} đổi {}{ph}{proposal}", c.ac),
                why: Some(clip(answer.trim(), 300)),
                reference: Some(a.id.clone()),
                run: run.map(|i| t.runs[i].log.clone()),
                who: String::new(),
            });
            continue;
        }
        // Already distilled into Key Decisions rows (`PO (board_ask Q-2)`): those rows
        // are the decision; they get the answer's time and run instead of a duplicate.
        let mut joined = false;
        for d in out.iter_mut().filter(|d| d.kind == "plan" && names_ask(&d.who, &a.id)) {
            if d.at.as_deref().is_none_or(|x| x.len() <= 10) {
                d.at = at.clone();
            }
            d.run = run.map(|i| t.runs[i].log.clone());
            d.reference = Some(format!("{} · {}", d.reference.as_deref().unwrap_or("PROJECT.md"), a.id));
            joined = true;
        }
        if joined {
            continue;
        }
        out.push(Decision {
            at,
            by: if a.by.as_deref() == Some("boss") { "boss" } else { "leader" },
            phase: run.and_then(|i| phases[i].clone()),
            kind: "answer",
            text: clip(answer.trim(), 600),
            why: Some(clip(a.question.trim(), 300)),
            reference: Some(a.id.clone()),
            run: run.map(|i| t.runs[i].log.clone()),
            who: String::new(),
        });
    }
    for (phase, notes) in progress::phase_notes(root, t) {
        let ended = phases.iter().enumerate().filter(|(_, p)| p.as_deref() == Some(phase.as_str())).filter_map(|(i, _)| t.runs[i].ended.clone()).max();
        // A re-grade the boss approved is already listed as his `ac_change` decision.
        let boss_ok = |ac: &str| t.asks.iter().any(|a| a.approved() == Some(true) && a.ac.as_ref().is_some_and(|c| c.ac == ac) && a.phase.as_deref().is_none_or(|p| p == phase));
        for n in notes.into_iter().filter(|n| n.status == "PASS" && !boss_ok(&n.id)) {
            let by = if n.source.contains("PO") || n.source.contains("Q-") { "leader" } else { "reviewer" };
            let q = n.source.split(['+', ',']).map(str::trim).find(|s| s.contains("Q-")).map(str::to_string);
            out.push(Decision {
                at: ended.clone(),
                by,
                phase: Some(phase.clone()),
                kind: "ac_regrade",
                text: format!("{}: {} ({})", n.id, n.status, n.note),
                why: q,
                reference: Some(format!("{phase}-VERIFICATION.md")),
                run: None,
                who: String::new(),
            });
        }
    }
    // ISO timestamps and bare dates sort together on their common prefix.
    out.sort_by(|a, b| match (&a.at, &b.at) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(stage: Stage, n: u32, started: &str, ended: &str, verdict: &str) -> Run {
        Run { stage, n, started: started.into(), ended: Some(ended.into()), exit: Some(0), cost: 1.0, verdict: Some(verdict.into()), log: format!("{}-{n}.jsonl", stage.key()), phase: None, round: None, title: None }
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ckit-timeline-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn phase_backfill_from_pm_ticks_and_neighbours() {
        let root = tmp("phase");
        std::fs::create_dir_all(root.join("agents/planning/board-t-001")).unwrap();
        let mut t = super::super::tests::task(Stage::Done, Status::Done);
        t.feature = Some("board-t-001".into());
        t.runs = vec![
            run(Stage::Intake, 1, "2026-10-15T10:00:00Z", "2026-10-15T10:03:00Z", "pass"),
            run(Stage::Solution, 1, "2026-10-15T10:03:01Z", "2026-10-15T10:30:00Z", "pass"),
            run(Stage::Review, 1, "2026-10-15T10:30:01Z", "2026-10-15T10:33:00Z", "pass"),
            run(Stage::Test, 1, "2026-10-15T11:00:00Z", "2026-10-15T11:30:00Z", "pass"),
            run(Stage::Solution, 2, "2026-10-15T11:30:02Z", "2026-10-15T11:33:00Z", "pass"),
            run(Stage::Review, 2, "2026-10-15T11:33:01Z", "2026-10-15T11:40:00Z", "fail"),
            run(Stage::Dev, 1, "2026-10-15T12:00:00Z", "2026-10-15T12:30:00Z", "pass"),
            run(Stage::Report, 1, "2026-10-15T13:00:00Z", "2026-10-15T13:02:00Z", "pass"),
        ];
        t.runs[6].phase = Some("M9".into()); // recorded at start wins
        let feed = [
            ("2026-10-15T10:05:00Z", "Leader + BA M0 đang solution — 1 phút"),
            ("2026-10-15T11:05:00Z", "Tester M0 đang test — 5 phút"),
            ("2026-10-15T11:25:00Z", "Tester M1 đang test — 25 phút"),
            ("2026-10-15T11:35:00Z", "Reviewer M1 đang review — 2 phút"),
            ("2026-10-15T12:05:00Z", "Dev M1 đang dev — 5 phút"),
        ];
        let lines: String = feed.iter().map(|(at, text)| format!("{}\n", serde_json::json!({ "at": at, "task": "T-001", "stage": "dev", "kind": "status", "text": text }))).collect();
        std::fs::create_dir_all(board_dir(&root)).unwrap();
        std::fs::write(board_dir(&root).join("pm.jsonl"), lines).unwrap();
        let got = run_phases(&root, &t);
        let want: Vec<Option<String>> = [None, Some("M0"), Some("M0"), Some("M0"), Some("M1"), Some("M1"), Some("M9"), None].iter().map(|p| p.map(String::from)).collect();
        assert_eq!(got, want, "review inherits the plan before it, a short plan the run after it, a ship run keeps its first tick");
        let _ = std::fs::remove_dir_all(&root);
    }

    fn msg(from: &str, text: &str, at: &str) -> Msg {
        Msg { from: from.into(), text: text.into(), at: at.into() }
    }

    #[test]
    fn thread_kinds_runs_and_unanswered_boss_notes() {
        let root = tmp("thread");
        let mut t = super::super::tests::task(Stage::Done, Status::Done);
        t.runs = vec![run(Stage::Solution, 1, "2026-10-15T10:00:00Z", "2026-10-15T10:30:00Z", "pass"), run(Stage::Dev, 1, "2026-10-15T11:00:00Z", "2026-10-15T11:30:00Z", "pass"), run(Stage::Report, 1, "2026-10-15T12:00:00Z", "2026-10-15T12:05:00Z", "pass")];
        t.thread = vec![
            msg("user", "[Bắt đầu]", "2026-10-15T09:59:00Z"),
            msg("solution → po", "[Q-1] A hay B?", "2026-10-15T10:05:00Z"),
            msg("po", "[Q-1] Chọn A", "2026-10-15T10:05:10Z"),
            msg("user → solution", "cái này phải gọi subagent ko", "2026-10-15T10:10:00Z"),
            msg("leader", "Kế hoạch xong", "2026-10-15T10:30:00Z"),
            msg("user", "Duyệt giải pháp.", "2026-10-15T11:00:00Z"),
            msg("coordinator → dev", "nhớ chạy test", "2026-10-15T11:10:00Z"),
            msg("user → dev", "dùng subagent", "2026-10-15T11:20:00Z"),
            msg("leader", "Báo cáo cuối", "2026-10-15T12:05:00Z"),
            msg("coordinator", "Duyệt kế hoạch thay sếp (theo luật uỷ quyền).", "2026-10-15T12:06:00Z"),
            msg("system", "Đã merge", "2026-10-15T12:07:00Z"),
            msg("user", "còn việc X thì sao?", "2026-10-15T12:08:00Z"),
        ];
        // The dev run received both notes; it acknowledged only the boss's.
        let dir = task_dir(&root, &t.id);
        let facts = digest::Facts {
            steers: vec![digest::Steer { text: "nhớ chạy test".into(), reply: None }, digest::Steer { text: "dùng subagent".into(), reply: Some("Đã nhận.".into()) }],
            ..Default::default()
        };
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        for r in &t.runs {
            std::fs::write(dir.join("logs").join(&r.log), "").unwrap();
        }
        digest::store(&dir, "dev-1.jsonl", 0, &facts);
        let mut v = serde_json::to_value(&t).unwrap();
        let story = enrich(&root, &t, &mut v);
        let kinds: Vec<&str> = v["thread"].as_array().unwrap().iter().map(|m| m["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["boss", "question", "answer", "boss", "leader", "boss", "coordinator", "boss", "report", "delegated_approval", "system", "boss"]);
        let runs: Vec<Option<&str>> = v["thread"].as_array().unwrap().iter().map(|m| m["run"].as_str()).collect();
        assert_eq!(runs[..9], [None, Some("solution-1.jsonl"), Some("solution-1.jsonl"), Some("solution-1.jsonl"), Some("solution-1.jsonl"), None, Some("dev-1.jsonl"), Some("dev-1.jsonl"), Some("report-1.jsonl")]);
        assert_eq!((v["thread"][3]["ack"].as_bool(), v["thread"][6]["ack"].as_bool(), v["thread"][7]["ack"].as_bool()), (Some(false), Some(true), Some(true)));
        assert_eq!(v["thread"][7]["reply"], "Đã nhận.");
        let un: Vec<&str> = story.unanswered.iter().map(|u| u["text"].as_str().unwrap()).collect();
        assert_eq!(un, ["cái này phải gọi subagent ko", "còn việc X thì sao?"], "undelivered note + free text nobody answered; acknowledged note and button actions are not");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn report_decide_list_item_with_sub_items_and_heading() {
        let md = "## Kết luận\n\n5. Xong.\n6. **Sếp cần quyết lúc nghiệm thu:**\n   - Có đưa `tests/Perf` lên master không. Nếu không:\n     `git rm --cached` 7 file.\n   - Backlog tie-breaker `crmid`.\n   - Các điểm đã **Loại**: #23.\n7. Hết.\n";
        assert_eq!(report_decide(md), ["Có đưa `tests/Perf` lên master không. Nếu không: `git rm --cached` 7 file.", "Backlog tie-breaker `crmid`.", "Các điểm đã Loại: #23."]);
        let md2 = "# R\n## Sếp cần quyết\n\n- [ ] Merge lên master?\n- Đổi tên API\n  (ảnh hưởng mobile)\n\n## Cách thử\n- x\n";
        assert_eq!(report_decide(md2), ["Merge lên master?", "Đổi tên API (ảnh hưởng mobile)"]);
        assert!(report_decide("## Kết luận\n- x\n").is_empty());
        assert_eq!(report_decide("**Sếp cần quyết:** có merge không\n"), ["có merge không"]);
    }

    #[test]
    fn summary_and_fail_reason_lines() {
        assert_eq!(summary_lines("**Kết quả:** xong\n\n- 3 file\n- 2 commit\n- 4th"), ["Kết quả: xong", "3 file", "2 commit"]);
        let long = format!("{}. Câu hai. Câu ba. Câu bốn.", "a".repeat(230));
        assert_eq!(summary_lines(&long).len(), 3);
        let md = "# REVIEW\n## Đã kiểm\n- ok\n## Blocking\n\n- **B1:** thiếu kiểm quyền non-admin\n## Non-blocking\n- x";
        assert_eq!(first_finding(md).as_deref(), Some("B1: thiếu kiểm quyền non-admin"));
        assert_eq!(first_finding("## Blocking\n\nKhông có.\n"), None);
    }

    #[test]
    fn key_decisions_table_maps_columns_and_owner() {
        let md = "## Key Decisions (table)\n\n> note\n\n| Ngày | Phase | Quyết định | Lý do | Ai quyết |\n|---|---|---|---|---|\n| 2026-10-15 | all | Chỉ server-side | đã đo | đề xuất |\n| 2026-10-16 | M0 | git add -f | harness | user (PO Q-1) |\n\n## Other\n| Ngày | Phase | Quyết định |\n|---|---|---|\n| x | y | z |\n";
        let d = key_decisions(md, "PROJECT.md");
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].at.as_deref(), d[0].phase.as_deref(), d[0].text.as_str(), d[0].why.as_deref(), d[0].by), (Some("2026-10-15"), Some("all"), "Chỉ server-side", Some("đã đo"), "leader"));
        assert_eq!(d[1].by, "boss");
    }

    fn ask(id: &str, at: &str, by: &str, answer: &str) -> ask::Ask {
        ask::Ask { id: id.into(), from: Stage::Solution, to: "po".into(), question: format!("{id}?"), options: Vec::new(), at: at.into(), status: "answered".into(), answer: Some(answer.into()), by: Some(by.into()), answered_at: Some(at.replace(":10Z", ":20Z")), kind: None, ac: None, phase: None }
    }

    #[test]
    fn decisions_merge_tables_gates_delegations_answers_and_regrades() {
        let root = tmp("decisions");
        let f = root.join("agents/planning/board-t-001");
        std::fs::create_dir_all(f.join("phases/M1-read")).unwrap();
        std::fs::write(f.join("ROADMAP.md"), "- [x] **M1 Read** — x\n").unwrap();
        std::fs::write(
            f.join("PROJECT.md"),
            "## Key Decisions\n| Ngày | Phase | Quyết định | Lý do | Ai quyết |\n|---|---|---|---|---|\n| 2026-10-15 | all | Chỉ server-side | đã đo | đề xuất |\n| 2026-10-15 | M1 | Nhận #10 | PO chốt | PO (board_ask Q-1) |\n| 2026-10-15 | M1 | Thêm user 209 | B1 | PO (REVIEW B1) |\n",
        )
        .unwrap();
        std::fs::write(f.join("phases/M1-read/M1-VERIFICATION.md"), "| UC | AC | Nguồn | Verdict |\n|---|---|---|---|\n| UC1 | AC-01 | REQ | PASS |\n| UC1 | AC-07 | REQUIREMENTS + PO Q-2 | PASS (AC chấm lại) |\n").unwrap();
        let mut t = super::super::tests::task(Stage::Done, Status::Done);
        t.feature = Some("board-t-001".into());
        t.runs = vec![run(Stage::Solution, 1, "2026-10-15T10:00:00Z", "2026-10-15T10:30:00Z", "pass"), run(Stage::Test, 1, "2026-10-15T11:00:00Z", "2026-10-15T11:30:00Z", "pass")];
        t.runs[0].phase = Some("M1".into());
        t.runs[1].phase = Some("M1".into());
        t.thread = vec![
            msg("coordinator", "Duyệt kế hoạch thay sếp (theo luật uỷ quyền).", "2026-10-15T10:31:00Z"),
            msg("user", "Duyệt giải pháp.", "2026-10-15T10:40:00Z"),
            msg("user", "Nghiệm thu.", "2026-10-15T12:00:00Z"),
        ];
        t.asks = vec![ask("Q-1", "2026-10-15T10:05:10Z", "po", "Nhận #10, chỉ trong helper"), ask("Q-2", "2026-10-15T11:05:10Z", "boss", "Chấm lại AC-07")];
        let d = decisions(&root, &t);
        let got: Vec<(&str, &str, &str, Option<&str>, Option<&str>)> = d.iter().map(|d| (d.kind, d.by, d.text.as_str(), d.phase.as_deref(), d.reference.as_deref())).collect();
        assert_eq!(
            got,
            [
                ("plan", "leader", "Chỉ server-side", Some("all"), Some("PROJECT.md")),
                ("plan", "leader", "Thêm user 209", Some("M1"), Some("PROJECT.md")),
                ("plan", "leader", "Nhận #10", Some("M1"), Some("PROJECT.md · Q-1")),
                ("delegated", "coordinator", "Duyệt kế hoạch thay sếp (theo luật uỷ quyền).", Some("M1"), None),
                ("gate", "boss", "Duyệt giải pháp.", Some("M1"), None),
                ("answer", "boss", "Chấm lại AC-07", Some("M1"), Some("Q-2")),
                ("ac_regrade", "leader", "AC-07: PASS (AC chấm lại)", Some("M1"), Some("M1-VERIFICATION.md")),
                ("gate", "boss", "Nghiệm thu.", None, None),
            ],
            "Q-1 is the PROJECT row it was distilled into (with the answer's time), not a second entry"
        );
        assert_eq!((d[2].at.as_deref(), d[2].run.as_deref()), (Some("2026-10-15T10:05:20Z"), Some("solution-1.jsonl")));
        assert_eq!(d[6].why.as_deref(), Some("PO Q-2"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn now_is_the_latest_pm_tick_of_the_running_run() {
        let root = tmp("now");
        let mut t = super::super::tests::task(Stage::Dev, Status::Running);
        let started = iso_at(SystemTime::now() - Duration::from_secs(600));
        t.runs = vec![Run { stage: Stage::Dev, n: 1, started: started.clone(), ended: None, exit: None, cost: 0.0, verdict: None, log: "dev-1.jsonl".into(), phase: Some("M1".into()), round: None, title: None }];
        let tick = |at: String, text: &str| format!("{}\n", serde_json::json!({ "at": at, "task": "T-001", "stage": "dev", "kind": "status", "text": text }));
        let feed = [
            tick(iso_at(SystemTime::now() - Duration::from_secs(900)), "Dev M0 đang dev — cũ"),
            tick(iso_at(SystemTime::now() - Duration::from_secs(300)), "Dev M1 đang dev — 5 phút, 12 lệnh"),
            tick(iso_at(SystemTime::now() - Duration::from_secs(5)), "Dev M1 đang dev — 10 phút, 20 lệnh"),
        ]
        .concat();
        std::fs::create_dir_all(board_dir(&root)).unwrap();
        std::fs::write(board_dir(&root).join("pm.jsonl"), feed).unwrap();
        let mut v = serde_json::to_value(&t).unwrap();
        let s = enrich(&root, &t, &mut v);
        let now = s.now.expect("running → now");
        assert_eq!((now["activity"].as_str(), now["stage"].as_str(), now["started"].as_str()), (Some("Dev M1 đang dev — 10 phút, 20 lệnh"), Some("dev"), Some(started.as_str())));
        assert!(v["runs"][0]["digest"].is_null(), "a running run has no digest yet");
        t.status = Status::Done;
        assert!(enrich(&root, &t, &mut serde_json::to_value(&t).unwrap()).now.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// No PM tick yet: `now.activity` comes from the log tail, `now.recent` lists the
    /// last 5 actions even when they sit behind > 64 KB of streaming deltas.
    #[test]
    fn now_falls_back_to_the_log_tail_and_lists_recent_actions() {
        let root = tmp("now-log");
        let mut t = super::super::tests::task(Stage::Solution, Status::Running);
        t.runs = vec![Run { stage: Stage::Solution, n: 1, started: iso_at(SystemTime::now() - Duration::from_secs(190)), ended: None, exit: None, cost: 0.0, verdict: None, log: "solution-1.jsonl".into(), phase: Some("M0".into()), round: None, title: None }];
        let start = |tool: &str, args: &str, intent: Option<&str>| {
            let intent = intent.map(|i| format!(",\"intent\":\"{i}\"")).unwrap_or_default();
            format!("{{\"type\":\"tool_execution_start\",\"toolCallId\":\"x\",\"toolName\":\"{tool}\",\"args\":{args}{intent}}}\n")
        };
        let delta = "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"delta\":\"".to_string() + &"x".repeat(200) + "\"}}\n";
        let mut log = String::from("{\"type\":\"session\",\"id\":\"s\"}\n");
        log += &start("read", "{\"path\":\"old.md\"}", Some("Old step"));
        for (tool, args, intent) in [
            ("read", "{\"path\":\"ROADMAP.md\"}", Some("Reading roadmap")),
            ("bash", "{\"command\":\"git status\"}", None),
            ("edit", "{\"path\":\"agents/planning/x/M0-CONTEXT.md\"}", Some("Writing context")),
            ("write", "{\"path\":\"xd://engine_plan\"}", Some("Planning engine")),
        ] {
            log += &start(tool, args, intent);
        }
        log += "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"timestamp\":1792086195530,\"content\":[{\"type\":\"text\",\"text\":\"Kế hoạch M0\\n đang viết\"}]}}\n";
        log += &delta.repeat(400); // ≈ 85 KB of deltas after the last action
        let logs = task_dir(&root, &t.id).join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("solution-1.jsonl"), log).unwrap();
        let now = enrich(&root, &t, &mut serde_json::to_value(&t).unwrap()).now.expect("running");
        let recent: Vec<(Option<&str>, &str)> = now["recent"].as_array().unwrap().iter().map(|a| (a["tool"].as_str(), a["text"].as_str().unwrap())).collect();
        assert_eq!(recent, [(Some("read"), "Reading roadmap"), (Some("bash"), "git status"), (Some("edit"), "Writing context"), (Some("write"), "Planning engine"), (None, "Kế hoạch M0 đang viết")]);
        assert_eq!(now["recent"][4]["at"], "2026-10-15T17:43:15Z");
        assert_eq!(now["files"], serde_json::json!(["agents/planning/x/M0-CONTEXT.md"]), "xd:// devices are not files");
        assert_eq!((now["source"].as_str(), now["activity"].as_str()), (Some("log"), Some("Leader · Kế hoạch M0 · 3 phút · đang: Kế hoạch M0 đang viết")));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Pause/resume buttons and delivered notes are never "chưa có phản hồi"; a run's
    /// summary prefers the Leader's thread message for it.
    #[test]
    fn actions_and_delivered_notes_are_not_unanswered_and_summary_prefers_thread() {
        let root = tmp("ack");
        let mut t = super::super::tests::task(Stage::Done, Status::Done);
        t.runs = vec![run(Stage::Dev, 1, "2026-10-15T11:00:00Z", "2026-10-15T11:30:00Z", "pass")];
        t.thread = vec![
            msg("user → dev", "[Tạm dừng] dừng để sửa môi trường", "2026-10-15T11:05:00Z"),
            msg("user → dev", "đổi sang user 209", "2026-10-15T11:10:00Z"),
            msg("user → dev", "còn file B?", "2026-10-15T11:15:00Z"),
            msg("user", "[Tiếp tục] làm tiếp", "2026-10-15T11:20:00Z"),
            msg("leader", "Dev xong M1: 21/21 task\n- 23 commit", "2026-10-15T11:30:00Z"),
            msg("system", "Đã merge", "2026-10-15T11:31:00Z"),
        ];
        let dir = task_dir(&root, &t.id);
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join("logs/dev-1.jsonl"), "").unwrap();
        digest::store(&dir, "dev-1.jsonl", 0, &digest::Facts { last_text: Some("English closing words".into()), ..Default::default() });
        std::fs::create_dir_all(dir.join("results")).unwrap();
        std::fs::write(dir.join("results/dev-1.json"), "{\"verdict\":\"pass\",\"message\":\"\"}").unwrap();
        // The engine took the second note (rename M-2.json → M-2.sent), no reply logged.
        let inbox = inbox::inbox_dir(&root, &t.id);
        std::fs::create_dir_all(&inbox).unwrap();
        std::fs::write(inbox.join("M-2.sent"), serde_json::json!({ "kind": "note", "text": "đổi sang user 209", "from": "boss" }).to_string()).unwrap();
        let mut v = serde_json::to_value(&t).unwrap();
        let s = enrich(&root, &t, &mut v);
        let th = v["thread"].as_array().unwrap();
        let flags: Vec<(bool, bool)> = th[..4].iter().map(|m| (m["action"].as_bool().unwrap(), m["ack"].as_bool().unwrap())).collect();
        assert_eq!(flags, [(true, false), (false, true), (false, false), (true, false)]);
        let un: Vec<&str> = s.unanswered.iter().map(|u| u["text"].as_str().unwrap()).collect();
        assert_eq!(un, ["còn file B?"], "only the undelivered free-text note waits for an answer");
        assert_eq!((v["runs"][0]["summary"].clone(), v["runs"][0]["summary_source"].as_str()), (serde_json::json!(["Dev xong M1: 21/21 task", "23 commit"]), Some("thread")));
        let _ = std::fs::remove_dir_all(&root);
    }
}
