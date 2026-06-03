# AeroFlow Frontend — Developer Manual

## Stack
- **React 19** + **TypeScript 5.8** — UI framework
- **Vite 6** — bundler and dev server
- **Tailwind CSS v4** — utility-first styling with `@theme` tokens in `index.css`
- **Lucide React** — icon library
- **Framer Motion** (`motion`) — animations
- **Express** — optional API server (not wired yet)

## Project Structure

```
frontend/
├── AeroFlow/              # Placeholder (empty)
├── dist/                  # Production build output
├── src/
│   ├── components/
│   │   ├── ActiveProjects.tsx  # Project list, detail drawer, CAD upload, AI chat
│   │   ├── AIPanel.tsx         # AI Insights panel, billing/privacy/earning sheets
│   │   ├── CFDSimulator.tsx    # Canvas-based 2D airfoil wind tunnel visualization
│   │   ├── DocGuide.tsx        # Aerodynamics equations reference
│   │   ├── Header.tsx          # Top bar: logo, search, notifications, settings
│   │   ├── ProfileSection.tsx  # User profile card with inline editor
│   │   └── Sidebar.tsx         # Left nav, quota meters, upgrade plan modal
│   ├── App.tsx           # Root component, all global state, layout
│   ├── data.ts           # Mock data (3 projects, 6 logs, 4 airfoils)
│   ├── index.css         # Tailwind imports, theme tokens, animations
│   ├── main.tsx          # Entry point
│   └── types.ts          # TypeScript interfaces + shared helpers
├── index.html
├── package.json
├── tsconfig.json
├── vite.config.ts
└── metadata.json         # Google AI Studio deployment metadata
```

## Architecture

### State Management
**All domain state lives in `App.tsx`** via `useState` hooks. There is no Redux, Zustand, or React Context.

| State | Type | Consumed By |
|---|---|---|
| `user` | `UserProfile` | Header, ProfileSection |
| `projects` | `Project[]` | Sidebar (quota), ActiveProjects, CFDSimulator, AIPanel |
| `logs` | `SystemLog[]` | AIPanel, inline log modal |
| `activeTab` | `string` | Sidebar, Header, main content switch |
| `selectedProjectForSolver` | `Project \| null` | CFDSimulator |
| `planQuota` | `object` | Sidebar |
| `showLogsModal` | `boolean` | inline log modal |

### Component Tree (1-level prop drilling)
```
App.tsx
├── Header          — props: user, onNavigate, activeTab
├── Sidebar         — props: activeTab, onNavigate, planQuota, onUpdatePlan
├── <main>
│   ├── CFDSimulator   — props: activeProject, allProjects, onSelectProject
│   ├── DocGuide       — no props (self-contained)
│   └── (dashboard)
│       ├── ProfileSection  — props: user, onUpdateUser
│       ├── ActiveProjects  — props: projects, onAdd/Update/Apply/Select
│       └── AIPanel         — props: suggestionAcceptanceRate, logs, onShowFullLogs
```

### Mock Data
All initial data is hardcoded in `data.ts`:
- 3 projects (Delta-7, Wing Assembly v4, Turbine Blade Cooling)
- 4 preset airfoils (NACA 4412, SC-02, Delta, MH 114)
- 6 system logs

No backend API is connected. The `/api` proxy is configured in `vite.config.ts` but unused.

## Key Conventions

### Types
All interfaces in `types.ts`:
- `AerodynamicParameters`, `AISuggestion`, `Project`, `UserProfile`, `SystemLog`
- Shared utility: `calcLiftDragRatio(aspect, mach, sweep)` for consistent L/D computation

### CSS/Theming
Tailwind v4 with custom theme tokens in `index.css`:
```
--color-brand-primary: #006194
--color-brand-accent:  #0284c7
--color-brand-bg:      #f9f9ff
--color-brand-text:    #111c2d
--color-brand-text-muted: #3f4850
```

Available animations: `animate-fade-in`, `animate-slide-up`, `animate-slide-in`.

### Icon Usage
All icons from `lucide-react`. Import only what you use:
```tsx
import { Folder, Plus, X, Cpu } from 'lucide-react';
```

### Local vs Global State Rule
- **Global domain data** (projects, user, logs, quotas) → `App.tsx` state, passed as props
- **UI transient state** (modals open/closed, form inputs, edit buffers, canvas refs) → local `useState`/`useRef` in each component

### Handler Pattern
Mutation handlers are defined in `App.tsx` and passed down as props:
```tsx
const handleUpdateProject = (updatedProj: Project) => {
  setProjects((prev) => prev.map((p) => p.id === updatedProj.id ? updatedProj : p));
};
```
Callbacks that do multiple mutations (project + quota + log) trigger each `setState` independently. Do NOT nest `setProjects` side effects inside other state updaters — the `makeLog` helper (`App.tsx`) handles log creation uniformly.

## Commands

| Command | Description |
|---|---|
| `npm run dev` | Dev server on `localhost:3000` with HMR |
| `npm run build` | Production build to `dist/` |
| `npx tsc --noEmit` | TypeScript type check (no output = clean) |
| `npm run preview` | Preview production build |
| `npm run lint` | Same as `tsc --noEmit` |

## Shared Helpers

### `calcLiftDragRatio(aspectRatio, machNumber, sweepAngle): number`
Single source of truth for L/D ratio. Formula: `aspect * 1.5 + mach * 4 - sweep * 0.04`.
Used in: `ActiveProjects.tsx` (init + 3 sliders).

### `makeLog(event, project, status, computeCost): SystemLog`
Factory for system log entries with auto-generated id/timestamp.
Used in: `App.tsx` (handleAddProject, handleApplyAISuggestion, handleUpdatePlan).

## Canvas Visualization (CFDSimulator.tsx)
- 2D airfoil wind tunnel using HTML Canvas (`requestAnimationFrame`)
- 4 preset airfoil geometries drawn via `quadraticCurveTo`/`bezierCurveTo`
- Custom CAD mode: emerald-colored 3D wireframe with structural ribs
- Particle system: 130 flow streamlines with laminar deflection / stall turbulence
- Stall detection at AoA > 13.5° (standard) or 15.5° (custom CAD)
- Effects: `useEffect` #1 syncs params from project, #2 computes Cl/Cd, #4 renders canvas

## Common Pitfalls

1. **Suggestion application** — `onApplyAISuggestion` only handles side effects (tokens + logs). The actual project mutation (applied flag, L/D recalc, convergence data) happens in `ActiveProjects.handleApplySuggestion`. Don't call `setProjects` in the App handler.

2. **L/D formula consistency** — Always use `calcLiftDragRatio()` from `types.ts`. Don't inline the formula manually with different coefficients.

3. **Log creation** — Use `makeLog()` helper. Don't construct `SystemLog` objects directly with manual timestamps.

4. **`projects[0]` references** — In the log modal terminal display, use `selectedProjectForSolver ?? projects[0]` instead of bare `projects[0]`.

5. **Unused lucide imports** — Tree-shaking is handled by Vite, but keep imports clean for readability. Remove unused icons.

6. **Tailwind custom colors** — Only use theme tokens (`brand-primary`, `brand-accent`, etc.) or standard Tailwind shades (50–900). Don't use values like `red-650` or `emerald-250`.

7. **No backend yet** — The `/api` proxy in `vite.config.ts` points to `localhost:9090` but no API server is running. All data is mock/hardcoded.
