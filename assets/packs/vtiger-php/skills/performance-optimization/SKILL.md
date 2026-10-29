---
name: performance-optimization
disable-model-invocation: true
user-invocable: true
description: Optimizes vtiger/CloudGO performance by measuring first, then routing to the matching profiling skill. Use when performance requirements exist, when you suspect a regression, or when page/endpoint load times need improvement.
---

# Performance Optimization

## Overview

Measure before optimizing. Profile first, identify the actual bottleneck, fix it, measure again. Optimize only what measurements prove matters. Coding rules for PHP/SQL performance live in `rule://performance-rules`; this skill only routes to the measuring tools.

## The Optimization Workflow

```
1. MEASURE  → Establish baseline with real data
2. IDENTIFY → Find the actual bottleneck (not assumed)
3. FIX      → Address the specific bottleneck
4. VERIFY   → Measure again, confirm improvement
5. GUARD    → Keep the harness/numbers so a regression is visible
```

## Where to Measure (vtiger/CloudGO)

| Symptom | Skill / tool |
|---|---|
| Slow page or AJAX locally (nginx/php-fpm/MySQL) | `skill://vtiger-cloudgo-perf-diagnosis` |
| Which PHP functions burn the time | `skill://xdebug-trigger-profile-cachegrind`, `skill://vtiger-local-php-cpu-profile` |
| Repeated DB/Redis calls per row (N+1) | `skill://vtiger-nplus1-sweep-origin-attribution`, `skill://database` |
| Slow in production after deploy | `skill://triage-vtiger-prod-slowness-from-logs` |
| Browser main thread / JS hotspots | `skill://vtiger-browser-main-thread-perf-profile` (omp `browser` tool) |
| Static assets (images, fonts, cache-busting) | `skill://vtiger-web-asset-perf-optimize` |
| Proposed cache/Redis fix | `skill://verify-cache-fix-touches-hotpath` |

SQL timing: `$dbconfig['log_sql']` (config.inc.php) or `SQL_LOG_INCLUDE_CALLER` (config.performance.php); see `skill://database` → references/performance.md.

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "We'll optimize later" | Performance debt compounds. Fix obvious anti-patterns now, defer micro-optimizations. |
| "It's fast on my machine" | Your machine isn't production. Profile with production-sized data. |
| "This optimization is obvious" | If you didn't measure, you don't know. Profile first. |

## Red Flags

- Optimization without profiling data to justify it
- N+1 query patterns (`getInstanceById()` / `pquery()` inside loops)
- List endpoints without pagination
- No before/after numbers in the hand-off

## Verification

After any performance-related change:

- [ ] Before and after measurements exist (specific numbers, same harness)
- [ ] The specific bottleneck is identified and addressed
- [ ] No N+1 queries in new data fetching code
- [ ] Output is unchanged (golden diff / smoke) — optimization didn't break behavior
