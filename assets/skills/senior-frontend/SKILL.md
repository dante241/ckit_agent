---
name: senior-frontend
description: Greenfield React/Next.js (App Router) + TypeScript + Tailwind engineering helpers. Use when scaffolding a new React/Next.js project, generating React component/hook boilerplate, analyzing a package.json bundle, or applying Next.js App Router performance patterns. Not for visual design/anti-slop (design-taste-frontend, impeccable) or vtiger/CloudGO jQuery/Smarty UI.
---

# Senior Frontend

Frontend development patterns, performance optimization, and automation tools for React/Next.js applications.

## Table of Contents

- [Project Scaffolding](#project-scaffolding)
- [Component Generation](#component-generation)
- [Bundle Analysis](#bundle-analysis)
- [React Patterns](#react-patterns)
- [Next.js Optimization](#nextjs-optimization)
- [Accessibility and Testing](#accessibility-and-testing)

---

## Project Scaffolding

Generate a new Next.js or React project with TypeScript, Tailwind CSS, and best practice configurations.

### Workflow: Create New Frontend Project

1. Run the scaffolder with your project name and template:
   ```bash
   python3 ~/.omp/skills/senior-frontend/scripts/frontend_scaffolder.py my-app --template nextjs
   ```

2. Optional feature flags (auth, api, forms, testing, storybook):
   ```bash
   python3 ~/.omp/skills/senior-frontend/scripts/frontend_scaffolder.py dashboard --template nextjs --features auth,api
   ```

3. Navigate to the project and install dependencies:
   ```bash
   cd my-app && npm install
   ```

4. Start the development server:
   ```bash
   npm run dev
   ```

### What the scaffolder actually produces

- **Base files are real**: layout, home page, `ui/` Button/Input/Card, layout Header/Footer/Sidebar, hooks, `cn()` utils, health route, configs.
- **Pinned stack is old**: `next ^14`, `react ^18`, `tailwindcss ^3.4` (v3 `postcss.config.js` + `@tailwind` directives), `eslint ^8` (`.eslintrc.json`). Current majors are Next 16 / React 19 / Tailwind v4. For a new project on the current stack, prefer `npx create-next-app@latest` and use this scaffold only as a folder-layout reference; Tailwind v4 setup follows `design-taste-frontend` §3.A.
- **`--features` only writes placeholder files**: every feature file (`lib/auth.ts`, `middleware.ts`, `lib/api-client.ts`, login/register forms, `vitest.config.ts`, `.storybook/*` …) contains just `// TODO: Implement <KEY>`. The feature packages are added to `dependencies` (not `devDependencies`) at version `"latest"`. Implement those files yourself or drop the flag; never ship the stubs.
- `app/(auth)/login` and `app/(auth)/register` pages are static "Authentication" headings, not working forms.

### Scaffolder Options

| Option | Description |
|--------|-------------|
| `--template nextjs` | Next.js 14 App Router project (default) |
| `--template react` | React 18 + Vite 5 with TypeScript |
| `--features auth\|api\|forms\|testing\|storybook` | Comma-separated; adds deps + placeholder files only (see above) |
| `--dir` | Output directory (default `.`) |
| `--list-templates` / `--list-features` | Print templates / features and exit |
| `--dry-run` | Preview files without creating them |
| `--json` | JSON output |

### Generated Structure (Next.js)

```
my-app/
├── app/
│   ├── layout.tsx        # Root layout with fonts
│   ├── page.tsx          # Home page
│   ├── globals.css       # Tailwind + CSS variables
│   ├── (auth)/login|register/page.tsx  # static placeholders
│   └── api/health/route.ts
├── components/
│   ├── ui/               # Button, Input, Card
│   └── layout/           # Header, Footer, Sidebar
├── hooks/                # useDebounce, useLocalStorage
├── lib/                  # utils (cn), constants
├── types/                # TypeScript interfaces
├── tailwind.config.ts
├── next.config.js
└── package.json
```

---

## Component Generation

Generate React components with TypeScript, tests, and Storybook stories.

### Workflow: Create a New Component

1. Generate a client component:
   ```bash
   python3 ~/.omp/skills/senior-frontend/scripts/component_generator.py Button --dir src/components/ui
   ```

2. Generate a server component:
   ```bash
   python3 ~/.omp/skills/senior-frontend/scripts/component_generator.py ProductCard --type server
   ```

3. Generate with test and story files:
   ```bash
   python3 ~/.omp/skills/senior-frontend/scripts/component_generator.py UserProfile --with-test --with-story
   ```

4. Generate a custom hook (writes `useFormValidation.ts`):
   ```bash
   python3 ~/.omp/skills/senior-frontend/scripts/component_generator.py FormValidation --type hook
   ```

Names may be PascalCase, kebab-case or snake_case (`product-card` → `ProductCard`).

### Generator Options

| Option | Description |
|--------|-------------|
| `--type client` | Client component with 'use client' (default) |
| `--type server` | Async server component |
| `--type hook` | Custom React hook |
| `--with-test` | Include test file |
| `--with-story` | Include Storybook story |
| `--no-index` | Skip the `index.ts` barrel |
| `--flat` | Create in output dir without subdirectory |
| `--dry-run` | Preview without creating files |

### Generated Component Example

The template is a generic wrapper. Replace the `<div>` with the element the component really renders (a clickable thing is a `<button>`, navigation is `<nav>`, etc.) before using it.

```tsx
'use client';

import { cn } from '@/lib/utils';

// Generic wrapper: swap <div> for the semantic element this renders
// (<button>, <nav>, <section>, <article>...) and add state/handlers as needed.
export interface ProductCardProps {
  className?: string;
  children?: React.ReactNode;
}

export function ProductCard({ className, children }: ProductCardProps) {
  return (
    <div className={cn('', className)}>
      {children}
    </div>
  );
}
```

---

## Bundle Analysis

Analyze package.json and project structure for bundle optimization opportunities.

### Workflow: Optimize Bundle Size

1. Run the analyzer on your project:
   ```bash
   python3 ~/.omp/skills/senior-frontend/scripts/bundle_analyzer.py /path/to/project
   ```

2. Review the health score and issues:
   ```
   Bundle Health Score: 75/100 (C)

   HEAVY DEPENDENCIES:
     moment (290KB)
       Alternative: date-fns (12KB) or dayjs (2KB)

     lodash (71KB)
       Alternative: lodash-es with tree-shaking
   ```

3. Apply the recommended fixes by replacing heavy dependencies.

4. Re-run with verbose mode to check import patterns:
   ```bash
   python3 ~/.omp/skills/senior-frontend/scripts/bundle_analyzer.py . --verbose
   ```

### Bundle Score Interpretation

| Score | Grade | Action |
|-------|-------|--------|
| 90-100 | A | Bundle is well-optimized |
| 80-89 | B | Minor optimizations available |
| 70-79 | C | Replace heavy dependencies |
| 60-69 | D | Multiple issues need attention |
| 0-59 | F | Critical bundle size problems |

### Heavy Dependencies Detected

The analyzer identifies these common heavy packages:

| Package | Size | Alternative |
|---------|------|-------------|
| moment | 290KB | date-fns (12KB) or dayjs (2KB) |
| lodash | 71KB | lodash-es with tree-shaking |
| axios | 14KB | Native fetch or ky (3KB) |
| jquery | 87KB | Native DOM APIs |
| @mui/material | Large | shadcn/ui or Radix UI |

---

## React Patterns

Reference: `references/react_patterns.md`

### Compound Components

Share state between related components through context. Every sub-component used in the JSX must be attached to the parent:

```tsx
'use client';

import { createContext, useContext, useId, useState } from 'react';

type TabsState = { active: number; setActive: (index: number) => void; id: string };
const TabsContext = createContext<TabsState | null>(null);

function useTabs() {
  const ctx = useContext(TabsContext);
  if (!ctx) throw new Error('Tabs.* must be rendered inside <Tabs>');
  return ctx;
}

function Tabs({ children }: { children: React.ReactNode }) {
  const [active, setActive] = useState(0);
  const id = useId();
  return (
    <TabsContext.Provider value={{ active, setActive, id }}>
      {children}
    </TabsContext.Provider>
  );
}

function TabList({ children }: { children: React.ReactNode }) {
  return <div role="tablist">{children}</div>;
}

function Tab({ index, children }: { index: number; children: React.ReactNode }) {
  const { active, setActive, id } = useTabs();
  return (
    <button
      type="button"
      role="tab"
      id={`${id}-tab-${index}`}
      aria-selected={active === index}
      aria-controls={`${id}-panel-${index}`}
      tabIndex={active === index ? 0 : -1}
      onClick={() => setActive(index)}
    >
      {children}
    </button>
  );
}

function TabPanel({ index, children }: { index: number; children: React.ReactNode }) {
  const { active, id } = useTabs();
  if (active !== index) return null;
  return (
    <div role="tabpanel" id={`${id}-panel-${index}`} aria-labelledby={`${id}-tab-${index}`}>
      {children}
    </div>
  );
}

Tabs.List = TabList;
Tabs.Tab = Tab;
Tabs.Panel = TabPanel;

// Usage
<Tabs>
  <Tabs.List>
    <Tabs.Tab index={0}>One</Tabs.Tab>
    <Tabs.Tab index={1}>Two</Tabs.Tab>
  </Tabs.List>
  <Tabs.Panel index={0}>Content 1</Tabs.Panel>
  <Tabs.Panel index={1}>Content 2</Tabs.Panel>
</Tabs>
```

For production tabs also add ArrowLeft/ArrowRight/Home/End roving focus (WAI-ARIA Tabs pattern), or use Radix `@radix-ui/react-tabs`.

### Custom Hooks

Extract reusable logic into `use*` hooks; `useDebounce`, `useLocalStorage`, `useMediaQuery` and friends are in `references/react_patterns.md` (Custom Hooks).

### Render Props

Share rendering logic:

```tsx
function DataFetcher({ url, render }) {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch(url).then(r => r.json()).then(setData).finally(() => setLoading(false));
  }, [url]);

  return render({ data, loading });
}

// Usage
<DataFetcher
  url="/api/users"
  render={({ data, loading }) =>
    loading ? <Spinner /> : <UserList users={data} />
  }
/>
```

---

## Next.js Optimization

Reference: `references/nextjs_optimization_guide.md`

### Server vs Client Components

Use Server Components by default. Add 'use client' only when you need:
- Event handlers (onClick, onChange)
- State (useState, useReducer)
- Effects (useEffect)
- Browser APIs

```tsx
// app/products/[id]/page.tsx - Server Component (default), no 'use client'
// Next 15+: params is a Promise
export default async function ProductPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const product = await getProduct(id);  // Server-side fetch

  return (
    <div>
      <h1>{product.name}</h1>
      <AddToCartButton productId={product.id} />  {/* Client component */}
    </div>
  );
}
```

```tsx
// components/add-to-cart-button.tsx - 'use client' must be the first line of its own file
'use client';

import { useState } from 'react';

export function AddToCartButton({ productId }: { productId: string }) {
  const [adding, setAdding] = useState(false);
  return (
    <button
      type="button"
      disabled={adding}
      onClick={async () => {
        setAdding(true);
        await addToCart(productId);
        setAdding(false);
      }}
    >
      Add
    </button>
  );
}
```

### Image Optimization

```tsx
import Image from 'next/image';

// Above the fold / LCP image: raise fetch priority
// (Next 16 deprecated `priority`; use fetchPriority="high", or `preload` when it must start in <head>)
<Image
  src="/hero.jpg"
  alt="Hero"
  width={1200}
  height={600}
  fetchPriority="high"
/>

// Responsive image with fill
<div className="relative aspect-video">
  <Image
    src="/product.jpg"
    alt="Product"
    fill
    sizes="(max-width: 768px) 100vw, 50vw"
    className="object-cover"
  />
</div>
```

### Data Fetching Patterns

```tsx
// Parallel fetching
async function Dashboard() {
  const [user, stats] = await Promise.all([
    getUser(),
    getStats()
  ]);
  return <div>...</div>;
}

// Streaming with Suspense
export default async function ProductPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  return (
    <div>
      <ProductDetails id={id} />
      <Suspense fallback={<ReviewsSkeleton />}>
        <Reviews productId={id} />
      </Suspense>
    </div>
  );
}
```

---

## Accessibility and Testing

Reference: `references/frontend_best_practices.md` (full checklist, ARIA, focus management)

### Accessibility Checklist

1. **Semantic HTML**: Use proper elements (`<button>`, `<nav>`, `<main>`)
2. **Keyboard Navigation**: All interactive elements focusable
3. **ARIA Labels**: Provide labels for icons and complex widgets
4. **Color Contrast**: Minimum 4.5:1 for normal text
5. **Focus Indicators**: Visible focus states

```tsx
// Accessible icon-only button (icon set per design-taste-frontend §3.C: Phosphor first)
import { XIcon } from '@phosphor-icons/react';

<button
  type="button"
  aria-label="Close dialog"
  onClick={onClose}
  className="focus-visible:ring-2 focus-visible:ring-blue-500"
>
  <XIcon aria-hidden="true" />
</button>

// Skip link for keyboard users
<a href="#main-content" className="sr-only focus:not-sr-only">
  Skip to main content
</a>
```

### Testing Strategy

```tsx
// Component test with React Testing Library
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

test('button triggers action on click', async () => {
  const onClick = vi.fn();
  render(<Button onClick={onClick}>Click me</Button>);

  await userEvent.click(screen.getByRole('button'));
  expect(onClick).toHaveBeenCalledTimes(1);
});

// Test accessibility
test('dialog is accessible', async () => {
  render(<Dialog open={true} title="Confirm" />);

  expect(screen.getByRole('dialog')).toBeInTheDocument();
  expect(screen.getByRole('dialog')).toHaveAttribute('aria-labelledby');
});
```

---

## Quick Reference

### Common Next.js Config

See `references/nextjs_optimization_guide.md` (Configuration Reference) for a full `next.config` with images, `optimizePackageImports`, headers and bundle analyzer.

### Tailwind CSS Utilities

```tsx
// Conditional classes with cn()
import { cn } from '@/lib/utils';

<button className={cn(
  'px-4 py-2 rounded',
  variant === 'primary' && 'bg-blue-500 text-white',
  disabled && 'opacity-50 cursor-not-allowed'
)} />
```

### TypeScript Patterns

```tsx
// Props with children
interface CardProps {
  className?: string;
  children: React.ReactNode;
}

// Generic component
interface ListProps<T> {
  items: T[];
  renderItem: (item: T) => React.ReactNode;
}

function List<T>({ items, renderItem }: ListProps<T>) {
  return <ul>{items.map(renderItem)}</ul>;
}
```

---

## Resources

- React Patterns: `references/react_patterns.md`
- Next.js Optimization: `references/nextjs_optimization_guide.md`
- Best Practices: `references/frontend_best_practices.md`
