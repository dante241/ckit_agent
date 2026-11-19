//! Adaptive model routing for omp.
//!
//! Reads `~/.config/8sync/models.toml` (falling back to the embedded default
//! `assets/configs/models.toml`), classifies a prompt into a task class, and
//! emits omp CLI flags (`--model` + `--plan`/`--smol`/`--slow`). omp owns the
//! actual model catalog and resolution (fuzzy match); 8sync only steers which
//! model omp uses per prompt instead of hard-fixing a single `default`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct ModelConfig {
    #[serde(default)]
    pub roles: Roles,
    #[serde(default)]
    pub tasks: BTreeMap<String, String>,
    /// Enable omp's `--advisor` passive per-turn reviewer. Default ON (skipped
    /// for trivial prompts to stay token-optimal). Opt out: top-level
    /// `advisor = false` in models.toml (`8sync harness model advisor off`), or
    /// `--no-advisor` on `8sync ai` / `8sync .` / `8sync new` for one launch.
    #[serde(default = "advisor_default")]
    pub advisor: bool,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            roles: Roles::default(),
            tasks: BTreeMap::new(),
            advisor: true,
        }
    }
}

fn advisor_default() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
pub struct Roles {
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub plan: String,
    #[serde(default)]
    pub smol: String,
    #[serde(default)]
    pub slow: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskClass {
    Plan,
    Review,
    Debug,
    Code,
    Trivial,
}

impl TaskClass {
    pub const ALL: [TaskClass; 5] = [TaskClass::Plan, TaskClass::Review, TaskClass::Debug, TaskClass::Code, TaskClass::Trivial];

    pub fn key(self) -> &'static str {
        match self {
            TaskClass::Plan => "plan",
            TaskClass::Review => "review",
            TaskClass::Debug => "debug",
            TaskClass::Code => "code",
            TaskClass::Trivial => "trivial",
        }
    }
}

impl ModelConfig {
    /// Load user config, else the embedded default, else an empty config (omp
    /// decides everything — graceful when nothing is configured).
    pub fn load() -> Self {
        if let Some(home) = dirs::home_dir() {
            let p = crate::brand::config_dir(&home).join("models.toml");
            if let Ok(s) = std::fs::read_to_string(&p) {
                if let Ok(c) = toml::from_str::<ModelConfig>(&s) {
                    return c;
                }
            }
        }
        crate::assets::read("configs/models.toml")
            .and_then(|s| toml::from_str::<ModelConfig>(&s).ok())
            .unwrap_or_default()
    }

    /// Model for a task class, falling back to `roles.default`.
    fn model_for(&self, class: TaskClass) -> &str {
        let task = self.tasks.get(class.key()).map(String::as_str).unwrap_or("");
        if task.is_empty() {
            self.roles.default.as_str()
        } else {
            task
        }
    }

    /// omp flags for a fresh prompt: `--model <classified>` + role flags.
    /// `override_model` (from `8sync ai --model X`) wins for the main model.
    /// Empty values are skipped so omp keeps its own defaults.
    pub fn omp_flags(&self, prompt: &str, override_model: Option<&str>) -> Vec<String> {
        let class = classify(prompt);
        let main = match override_model {
            Some(m) if !m.trim().is_empty() => m.trim().to_string(),
            _ => self.model_for(class).to_string(),
        };
        self.flags_with(class, &main)
    }

    /// omp flags for a known task class (no prompt heuristics) — used by the
    /// board pipeline, where each stage's role already fixes the class. Carries
    /// an explicit `--thinking` (see [`Self::class_flags_with`]).
    pub fn class_flags(&self, class: TaskClass) -> Vec<String> {
        let omp = dirs::home_dir().map(|h| OmpRoles::load(&h.join(".omp/agent/config.yml"))).unwrap_or_default();
        self.class_flags_with(class, &omp)
    }

    /// [`Self::class_flags`] plus `--thinking <level>` from the class's resolved
    /// model (`@role` → omp `modelRoles` value, or a model id, with a `:level`
    /// suffix): a `--continue`d session otherwise keeps the thinking level of
    /// whatever step saved it (a /cook run resuming its plan session at `high`).
    pub fn class_flags_with(&self, class: TaskClass, omp: &OmpRoles) -> Vec<String> {
        let mut out = self.flags_with(class, self.model_for(class));
        if let Some(level) = self.resolve(class, omp).thinking {
            out.push("--thinking".to_string());
            out.push(level);
        }
        out
    }

    fn flags_with(&self, class: TaskClass, main: &str) -> Vec<String> {
        let mut out = Vec::new();
        push_flag(&mut out, "--model", main);
        self.push_role_flags(&mut out);
        // Advisor: passive per-turn rule/tool reviewer. On for substantive work,
        // skipped for trivial prompts to stay token-optimal.
        if self.advisor && class != TaskClass::Trivial {
            out.push("--advisor".to_string());
        }
        out
    }

    /// Role flags for resume/continue, where there is no new prompt to classify.
    ///
    /// Intentionally does NOT emit `--model`: on `--continue` omp restores the
    /// model saved in the session, and a CLI `--model` would override it every
    /// launch — so `8sync .` would silently reset the user's in-session model
    /// pick back to `roles.default` each time. Role flags (`--plan/--smol/--slow`)
    /// only steer omp's own routing and are safe to pass.
    pub fn resume_flags(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.push_role_flags(&mut out);
        // Interactive dev session (`8sync .` / resume): advisor on.
        if self.advisor {
            out.push("--advisor".to_string());
        }
        out
    }

    fn push_role_flags(&self, out: &mut Vec<String>) {
        push_flag(out, "--plan", &self.roles.plan);
        push_flag(out, "--smol", &self.roles.smol);
        push_flag(out, "--slow", &self.roles.slow);
    }
}

fn push_flag(out: &mut Vec<String>, flag: &str, val: &str) {
    let v = val.trim();
    if !v.is_empty() {
        out.push(flag.to_string());
        out.push(v.to_string());
    }
}

/// omp's own `modelRoles` map + the `task.agentModelOverrides.reviewer`
/// override, read line-based from `~/.omp/agent/config.yml` (no YAML dep).
#[derive(Debug, Default)]
pub struct OmpRoles {
    pub roles: BTreeMap<String, String>,
    pub reviewer: Option<String>,
}

impl OmpRoles {
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path).map(|raw| Self::parse(&raw)).unwrap_or_default()
    }

    pub fn parse(raw: &str) -> Self {
        let clean = |v: &str| {
            let v = v.split(" #").next().unwrap_or("").trim();
            v.trim_matches(|c| c == '"' || c == '\'').to_string()
        };
        let mut out = Self::default();
        let mut in_roles = false;
        for line in raw.lines() {
            if !line.starts_with([' ', '\t']) {
                in_roles = line.trim_start().starts_with("modelRoles:");
                continue;
            }
            let t = line.trim();
            if t.starts_with('#') {
                continue;
            }
            if in_roles {
                if let Some((k, v)) = t.split_once(':') {
                    let v = clean(v);
                    if !v.is_empty() {
                        out.roles.insert(k.trim().to_string(), v);
                    }
                }
            } else if let Some(rv) = t.strip_prefix("reviewer:") {
                let rv = clean(rv);
                if !rv.is_empty() {
                    out.reviewer = Some(rv);
                }
            }
        }
        out
    }
}

/// How a model setting reaches a concrete model: the raw value ckit passes
/// (`@task`, a model id, or "" = no `--model`), the model omp ends up using
/// (`None` = unknown), and a short Vietnamese explanation chain.
#[derive(Debug, Serialize, PartialEq)]
pub struct Resolved {
    pub setting: String,
    pub model: Option<String>,
    pub via: String,
    /// Thinking level the run gets (`--thinking`), from the model's `:level`.
    pub thinking: Option<String>,
}

/// omp's thinking levels, as accepted by `--thinking` and model `:level` suffixes.
const THINKING: [&str; 8] = ["off", "minimal", "low", "medium", "high", "xhigh", "max", "auto"];

/// `anthropic/claude-opus-5-5:medium` → `medium`.
pub fn thinking_of(model: &str) -> Option<String> {
    let (_, level) = model.trim().rsplit_once(':')?;
    THINKING.contains(&level).then(|| level.to_string())
}

impl ModelConfig {
    /// Resolve the `--model` a task class gets ([`Self::class_flags`]) through
    /// omp `modelRoles`.
    pub fn resolve(&self, class: TaskClass, omp: &OmpRoles) -> Resolved {
        let k = class.key();
        let task = self.tasks.get(k).map(|s| s.trim()).unwrap_or("");
        let def = self.roles.default.trim();
        let (setting, via) = if !task.is_empty() {
            (task, format!("models.toml {k}={task}"))
        } else if !def.is_empty() {
            (def, format!("models.toml {k} trống → roles.default={def}"))
        } else {
            ("", format!("models.toml {k} trống"))
        };
        resolve_setting(setting, via, omp)
    }

    /// The `--advisor` reviewer omp runs per turn (omp role `advisor`).
    pub fn resolve_advisor(omp: &OmpRoles) -> Resolved {
        match omp.roles.get("advisor") {
            Some(m) => Resolved { setting: "@advisor".into(), model: Some(m.clone()), via: "advisor=true → omp advisor".into(), thinking: thinking_of(m) },
            None => resolve_setting("", "advisor=true, omp chưa đặt advisor".into(), omp),
        }
    }
}

fn resolve_setting(setting: &str, via: String, omp: &OmpRoles) -> Resolved {
    let (model, tail) = if let Some(role) = setting.strip_prefix('@') {
        match omp.roles.get(role) {
            Some(m) => (Some(m.clone()), format!("omp {role}")),
            None => (None, format!("omp không có role {role}")),
        }
    } else if !setting.is_empty() {
        (Some(setting.to_string()), "model id trực tiếp".to_string())
    } else {
        match omp.roles.get("default") {
            Some(m) => (Some(m.clone()), "omp default".to_string()),
            None => (None, "omp tự chọn (chưa đặt modelRoles.default)".to_string()),
        }
    };
    let thinking = model.as_deref().and_then(thinking_of);
    Resolved { setting: setting.to_string(), model, via: format!("{via} → {tail}"), thinking }
}

/// Heuristic prompt → task class. Specific intents (review/plan/debug) beat the
/// generic `code` default; a very short prompt with no build verb is `trivial`.
pub fn classify(prompt: &str) -> TaskClass {
    let p = prompt.to_lowercase();
    let has = |kws: &[&str]| kws.iter().any(|k| p.contains(k));

    if has(&["review", "audit", "critique", "vulnerab", "security", "code smell"]) {
        return TaskClass::Review;
    }
    if has(&[
        "plan", "architect", "design ", "approach", "strategy", "how should",
        "trade-off", "tradeoff", "decompose",
    ]) {
        return TaskClass::Plan;
    }
    if has(&[
        "debug", "fix ", "bug", "error", "crash", "failing", "stack trace",
        "why does", "why is", "broken", "regression",
    ]) {
        return TaskClass::Debug;
    }
    let build_verb = has(&["implement", "build", "add ", "refactor", "write", "create", "migrate"]);
    if p.split_whitespace().count() <= 4 && !build_verb {
        return TaskClass::Trivial;
    }
    TaskClass::Code
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A resumed session must not keep another step's thinking level: board
    /// flags carry `--thinking` from the class's resolved model.
    #[test]
    fn board_flags_carry_the_classes_thinking_level() {
        let omp = OmpRoles::parse("modelRoles:\n  task: anthropic/claude-opus-5-5:medium\n  plan: anthropic/claude-opus-5-5:high\n  default: anthropic/claude-sonnet-5\n");
        let cfg: ModelConfig = toml::from_str("[tasks]\ncode = \"@task\"\nplan = \"@plan\"\nreview = \"openai/gpt-6:low\"\ntrivial = \"@nope\"\n").unwrap();
        let level = |c| {
            let f = cfg.class_flags_with(c, &omp);
            f.iter().position(|a| a == "--thinking").map(|i| f[i + 1].clone())
        };
        assert_eq!(level(TaskClass::Code).as_deref(), Some("medium"));
        assert_eq!(level(TaskClass::Plan).as_deref(), Some("high"));
        assert_eq!(level(TaskClass::Review).as_deref(), Some("low"), "explicit model id with :level");
        assert_eq!(level(TaskClass::Trivial), None, "unknown role: omit");
        assert_eq!(level(TaskClass::Debug), None, "omp default without a level: omit");
        assert!(cfg.class_flags_with(TaskClass::Code, &omp).windows(2).any(|w| w == ["--model", "@task"]));
        assert_eq!(cfg.resolve(TaskClass::Code, &omp).thinking.as_deref(), Some("medium"));
        assert_eq!(thinking_of("x/y:latest"), None);
    }

    #[test]
    fn classify_intents() {
        assert_eq!(classify("review the auth module for security holes"), TaskClass::Review);
        assert_eq!(classify("plan the architecture for a job queue"), TaskClass::Plan);
        assert_eq!(classify("fix the failing login test"), TaskClass::Debug);
        assert_eq!(classify("implement a dark mode toggle"), TaskClass::Code);
        assert_eq!(classify("rename foo"), TaskClass::Trivial);
    }

    #[test]
    fn embedded_default_parses_and_routes() {
        let cfg: ModelConfig = toml::from_str(
            r#"
            [roles]
            default = "codex"
            plan = "glm"
            smol = "haiku"
            slow = "opus"
            [tasks]
            plan = "glm"
            review = "opus"
            code = "codex"
            "#,
        )
        .unwrap();
        // review prompt → opus, role flags appended.
        let f = cfg.omp_flags("audit this for vulnerabilities", None);
        assert!(f.windows(2).any(|w| w == ["--model", "opus"]));
        assert!(f.windows(2).any(|w| w == ["--plan", "glm"]));
        // explicit override wins.
        let f2 = cfg.omp_flags("plan something", Some("glm"));
        assert!(f2.windows(2).any(|w| w == ["--model", "glm"]));
    }

    #[test]
    fn resolves_through_omp_model_roles() {
        let omp = OmpRoles::parse(
            "theme: dark\nmodelRoles:\n  task: anthropic/claude-opus-5-5:medium\n  # note\n  default: \"zai/glm-5.2\"\n  advisor: x/adv\nstatusLine:\n  preset: custom\ntask:\n  agentModelOverrides:\n    reviewer: x/rev:high\n",
        );
        assert_eq!(omp.roles.len(), 3);
        assert_eq!(omp.reviewer.as_deref(), Some("x/rev:high"));
        let cfg: ModelConfig = toml::from_str("[tasks]\ncode = \"@task\"\nreview = \"@nope\"\nplan = \"my/model\"\n").unwrap();
        let r = cfg.resolve(TaskClass::Code, &omp);
        assert_eq!(r.setting, "@task");
        assert_eq!(r.model.as_deref(), Some("anthropic/claude-opus-5-5:medium"));
        assert_eq!(r.via, "models.toml code=@task → omp task");
        assert_eq!(cfg.resolve(TaskClass::Review, &omp).model, None);
        assert_eq!(cfg.resolve(TaskClass::Plan, &omp).model.as_deref(), Some("my/model"));
        // Empty class + empty roles.default → no --model → omp default.
        assert_eq!(cfg.resolve(TaskClass::Debug, &omp).model.as_deref(), Some("zai/glm-5.2"));
        assert_eq!(ModelConfig::resolve_advisor(&omp).model.as_deref(), Some("x/adv"));
    }
}
