/** Logo « Time Marker » sur fond indigo (source : design/brand/trackify-sidebar-mark.svg). */
export function BrandMark({ size = 24 }: { size?: number }) {
  return (
    <svg className="brand-mark" width={size} height={size} viewBox="0 0 256 256" aria-hidden>
      <rect width="256" height="256" rx="64" fill="url(#brand-bg)" />
      <defs>
        <linearGradient id="brand-bg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#6872F2" />
          <stop offset="1" stopColor="#414BC4" />
        </linearGradient>
      </defs>
      <rect x="42" y="112" width="48" height="104" rx="24" fill="#B9B3FF" />
      <rect x="108" y="58" width="48" height="158" rx="24" fill="#FFFFFF" />
      <circle cx="190" cy="54" r="25" fill="#FF6476" />
    </svg>
  );
}
