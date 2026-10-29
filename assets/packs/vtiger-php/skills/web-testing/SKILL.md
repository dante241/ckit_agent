---
name: web-testing
disable-model-invocation: true
user-invokable: true
description: Web testing with Playwright, Vitest, k6. E2E/unit/integration/load/security/visual/a11y testing. Use for test automation, flakiness, Core Web Vitals, mobile gestures, cross-browser.
license: Apache-2.0
version: 3.0.0
---

# Web Testing Skill

Comprehensive web testing: unit, integration, E2E, load, security, visual regression, accessibility.

> **Scope: generic / greenfield Node projects.** Commands below assume a Node toolchain (`package.json`, Playwright/Vitest config) and GitHub Actions. The vtiger repo has none of these: it uses `.gitlab-ci.yml` and `phpunit.xml.dist` (covers `domains/*/tests` only). For vtiger work use the `testing` skill (PHPUnit / standalone scripts), `k6-loadtest-vtiger-readonly` + `test/load-test/` for load tests, and the omp `browser` eval global for UI checks.

## Quick Start

```bash
npx vitest run                    # Unit tests
npx playwright test               # E2E tests
npx playwright test --ui          # E2E with UI
k6 run load-test.js               # Load tests
npx @axe-core/cli https://example.com  # Accessibility
npx lighthouse https://example.com     # Performance
```

## Testing Strategy (Choose Your Model)

| Model | Structure | Best For |
|-------|-----------|----------|
| Pyramid | Unit 70% > Integration 20% > E2E 10% | Monoliths |
| Trophy | Integration-heavy | Modern SPAs |
| Honeycomb | Contract-centric | Microservices |

→ `./references/testing-pyramid-strategy.md`

## Reference Documentation

### Core Testing
- `./references/unit-integration-testing.md` - Vitest, browser mode, AAA
- `./references/e2e-testing-playwright.md` - Fixtures, sharding, selectors
- `./references/playwright-component-testing.md` - CT patterns (production-ready)
- `./references/component-testing.md` - React/Vue/Angular patterns

### Test Infrastructure
- `./references/test-data-management.md` - Factories, fixtures, seeding
- `./references/database-testing.md` - Testcontainers, transactions
- `./references/ci-cd-testing-workflows.md` - GitHub Actions, sharding
- `./references/contract-testing.md` - Pact, MSW patterns
- `./references/interactive-testing-patterns.md` - Forms, keyboard, drag-and-drop, hover/modals
- `./references/shadow-dom-testing.md` - Web Components / shadow DOM selectors

### Cross-Browser & Mobile
- `./references/cross-browser-checklist.md` - Browser/device matrix
- `./references/mobile-gesture-testing.md` - Touch, swipe, orientation

### Performance & Quality
- `./references/performance-core-web-vitals.md` - LCP/CLS/INP, Lighthouse CI
- `./references/visual-regression.md` - Screenshot comparison
- `./references/test-flakiness-mitigation.md` - Stability strategies

### Accessibility & Security
- `./references/accessibility-testing.md` - WCAG, axe-core
- `./references/security-testing-overview.md` - OWASP Top 10
- `./references/security-checklists.md` - Auth, API, headers
- `./references/vulnerability-payloads.md` - SQLi/XSS/etc. test payloads

### API & Load
- `./references/api-testing.md` - Supertest, GraphQL
- `./references/load-testing-k6.md` - k6 patterns (vtiger: use `k6-loadtest-vtiger-readonly` + `test/load-test/`)

### Checklists
- `./references/pre-release-checklist.md` - Complete release checklist
- `./references/functional-testing-checklist.md` - Feature testing

## Scripts

### Initialize Playwright Project
```bash
node ./scripts/init-playwright.js [--ct] [--dir <path>]
```
Creates best-practice Playwright setup: config, fixtures, example tests.

### Analyze Test Results
```bash
node ./scripts/analyze-test-results.js \
  --playwright test-results/results.json \
  --vitest coverage/vitest.json \
  --output markdown
```
Parses Playwright/Vitest/JUnit results into unified summary.

## CI/CD Integration (GitHub Actions example; vtiger uses `.gitlab-ci.yml`)

```yaml
jobs:
  test:
    steps:
      - run: npm run test:unit      # Gate 1: Fast fail
      - run: npm run test:e2e       # Gate 2: After unit pass
      - run: npm run test:a11y      # Accessibility
      - run: npx lhci autorun       # Performance
```
