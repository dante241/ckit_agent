// Embed git commit at build time so the binary can compare against upstream.
fn main() {
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o.stdout) } else { None })
        .and_then(|b| String::from_utf8(b).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", commit);

    // The `ckit harness web` dashboard (Vite FE) is NOT auto-built here: a plain
    // `cargo build` never shells out to bun/pnpm/npm, so it can't run package
    // lifecycle scripts or touch your npm/registry auth. Build a fresh bundle
    // explicitly when you want one — `pnpm -C web install && pnpm -C web build`
    // (release CI does this in a dedicated step). rust-embed then embeds whatever
    // web/dist holds, and we drop in an instructive fallback when it was never built.
    let web_dist = std::path::Path::new("../../web/dist/index.html");
    if !web_dist.exists() {
        if let Some(p) = web_dist.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let _ = std::fs::write(web_dist, FALLBACK_HTML);
        println!("cargo:warning=ckit: web/dist not built — embedded fallback page; run `pnpm -C web install && pnpm -C web build` for the full dashboard");
    }
    println!("cargo:rerun-if-changed=../../web/dist/index.html");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads/main");
}

const FALLBACK_HTML: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>ckit harness</title><style>
:root{color-scheme:dark}*{box-sizing:border-box}
body{margin:0;min-height:100vh;display:grid;place-items:center;font:15px/1.6 system-ui,-apple-system,sans-serif;
color:#e6e9ef;background:radial-gradient(1100px 760px at 72% -12%,#1b1f3a,#0b0d12)}
.card{max-width:540px;margin:24px;padding:34px 38px;border-radius:18px;background:rgba(20,23,31,.62);
backdrop-filter:blur(14px);-webkit-backdrop-filter:blur(14px);border:1px solid rgba(255,255,255,.08);
box-shadow:0 24px 70px rgba(0,0,0,.55)}
h1{margin:0 0 10px;font-size:20px;letter-spacing:.2px}p{color:#9aa3b2;margin:8px 0}
code{background:rgba(124,92,255,.14);padding:2px 8px;border-radius:7px;font:13px ui-monospace,Menlo,monospace;color:#b9c0ff}</style>
</head><body><div class="card"><h1>ckit harness · dashboard</h1>
<p>The web frontend was not compiled into this binary (no JS toolchain at build time).</p>
<p>Install <code>bun</code> and rebuild:</p>
<p><code>curl -fsSL https://bun.sh/install | bash</code></p>
<p><code>bun --cwd web install &amp;&amp; bun --cwd web run build</code></p>
<p><code>cargo build --release</code></p></div></body></html>"#;
