// Ckit Harness logo mark — 24-grid glyphs drawn in currentColor (the tile
// behind it is CSS: .brand-mark). Concepts live side by side so swapping is a
// one-word change to BRAND: A = C+H connector (shipped), B = board + agent,
// C = 'Ck' squircle monogram. Favicon: web/public/favicon.svg (concept A).
import { useId, type ReactNode } from "react";

type Concept = "A" | "B" | "C";
export const BRAND: Concept = "A";

const STROKE = { fill: "none", stroke: "currentColor", strokeWidth: 2.25, strokeLinecap: "round", strokeLinejoin: "round" } as const;

const MARKS: Record<Concept, (id: string) => ReactNode> = {
  // 'C' socket + 'H' plug docked into its mouth: the H's left leg closes the C.
  A: () => (
    <g {...STROKE}>
      <path d="M13.2 6.3A6.5 6.5 0 1 0 13.2 17.7" />
      <path d="M15.5 7v10M15.5 12h4.5M20 8.5v7" />
    </g>
  ),
  // A framed kanban: two cards and one live agent node.
  B: () => (
    <>
      <rect x="3.5" y="3.5" width="17" height="17" rx="4" {...STROKE} />
      <g fill="currentColor">
        <rect x="7" y="7" width="4" height="10" rx="1.5" />
        <rect x="13" y="7" width="4" height="4.5" rx="1.5" />
        <circle cx="15" cy="15" r="2" />
      </g>
    </>
  ),
  // 'Ck' knocked out of a squircle; the k's arm ends in a link node.
  C: (id) => (
    <>
      <mask id={id}>
        <rect width="24" height="24" fill="#fff" />
        <g {...STROKE} stroke="#000">
          <path d="M11.2 8.7A4.4 4.4 0 1 0 11.2 15.3" />
          <path d="M14 6.5v11M14 14l2.6-2.6M15.6 12.9l2.6 4.6" />
        </g>
        <circle cx="18.4" cy="9.4" r="1.6" fill="#000" />
      </mask>
      <rect x="2" y="2" width="20" height="20" rx="6.5" fill="currentColor" mask={`url(#${id})`} />
    </>
  ),
};

export function BrandMark({ concept = BRAND, size = 24 }: { concept?: Concept; size?: number }) {
  const id = useId();
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {MARKS[concept](id)}
    </svg>
  );
}
