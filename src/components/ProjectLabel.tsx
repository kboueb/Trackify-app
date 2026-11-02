export function ColorDot({ color, live = false }: { color: string; live?: boolean }) {
  return <span className={live ? "dot live" : "dot"} style={{ background: color, color }} aria-hidden />;
}
