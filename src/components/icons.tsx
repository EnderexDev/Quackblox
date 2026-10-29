import type { SVGProps } from "react";

type P = SVGProps<SVGSVGElement> & { size?: number };

const Icon = ({ size = 20, children, ...p }: P) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.9} strokeLinecap="round" strokeLinejoin="round" aria-hidden {...p}>
    {children}
  </svg>
);

/** Quackify's duck. */
export const DuckIcon = ({ size = 20, ...p }: P) => (
  <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden {...p}>
    <path
      fill="#ffd84d"
      d="M38 10c7.7 0 14 6.1 14 13.7 0 3.1-1 5.9-2.8 8.2C56.7 35 61 41 61 47.6 61 56.6 52.4 62 41 62H20C9.5 62 3 55.8 3 47.4c0-6.8 5.5-11.6 12.3-11.6 3.2 0 6 1 8.1 2.9-.7-2-1.1-4.1-1.1-6.4C22.3 18.2 29.3 10 38 10z"
    />
    <path fill="#ff9f1c" d="M50 21.5c4.8-.6 9.2.4 11 2.6-1.8 2.6-6.4 4-11.6 3.6z" />
    <circle cx="42" cy="20" r="3" fill="#1a1a1a" />
    <path fill="#f2c230" d="M20 44c6 6 18 7 26 1-3 8-13 11-21 8-4-1.5-6-5-5-9z" />
  </svg>
);

export const PlayIcon = ({ size = 20, ...p }: P) => (
  <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden {...p}>
    <path fill="currentColor" d="M8 5.5v13a1 1 0 0 0 1.5.86l10.6-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z" />
  </svg>
);

export const PlusIcon = (p: P) => (
  <Icon {...p}>
    <path d="M12 5v14M5 12h14" />
  </Icon>
);

export const MoreIcon = ({ size = 20, ...p }: P) => (
  <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden {...p}>
    <g fill="currentColor">
      <circle cx="5.5" cy="12" r="1.8" />
      <circle cx="12" cy="12" r="1.8" />
      <circle cx="18.5" cy="12" r="1.8" />
    </g>
  </svg>
);

export const SearchIcon = (p: P) => (
  <Icon {...p}>
    <circle cx="11" cy="11" r="6.5" />
    <path d="m20 20-4.2-4.2" />
  </Icon>
);

export const CloseIcon = (p: P) => (
  <Icon {...p}>
    <path d="M6 6l12 12M18 6 6 18" />
  </Icon>
);

export const CheckIcon = (p: P) => (
  <Icon {...p}>
    <path d="m5 12.5 4.5 4.5L19 7.5" />
  </Icon>
);

export const TrashIcon = (p: P) => (
  <Icon {...p}>
    <path d="M4.5 7h15M10 11v6M14 11v6M6.5 7l.8 11.2A2 2 0 0 0 9.3 20h5.4a2 2 0 0 0 2-1.8L17.5 7M9.5 7V5.5A1.5 1.5 0 0 1 11 4h2a1.5 1.5 0 0 1 1.5 1.5V7" />
  </Icon>
);

export const ArrowUpIcon = (p: P) => (
  <Icon {...p}>
    <path d="M12 19V5M6 11l6-6 6 6" />
  </Icon>
);

export const LoginIcon = (p: P) => (
  <Icon {...p}>
    <path d="M14 4h3.5A2.5 2.5 0 0 1 20 6.5v11a2.5 2.5 0 0 1-2.5 2.5H14M10 16l4-4-4-4M14 12H4" />
  </Icon>
);

export const PcIcon = (p: P) => (
  <Icon {...p}>
    <rect x="3" y="4.5" width="18" height="12" rx="2" />
    <path d="M9 20h6M12 16.5V20" />
  </Icon>
);

export const FolderIcon = (p: P) => (
  <Icon {...p}>
    <path d="M3.5 7.4a1.9 1.9 0 0 1 1.9-1.9h3.7l2 2h7.5a1.9 1.9 0 0 1 1.9 1.9v8.2a1.9 1.9 0 0 1-1.9 1.9H5.4a1.9 1.9 0 0 1-1.9-1.9z" />
  </Icon>
);

export const UsersIcon = (p: P) => (
  <Icon {...p}>
    <circle cx="9" cy="8.5" r="3.5" />
    <path d="M2.5 19.5c.6-3.3 3.3-5.5 6.5-5.5s5.9 2.2 6.5 5.5M16 5.2a3.5 3.5 0 0 1 0 6.6M18.5 14.4c1.7.8 2.8 2.6 3.1 5.1" />
  </Icon>
);

export const LeafIcon = (p: P) => (
  <Icon {...p}>
    <path d="M5 19c0-8 5-14 15-14 0 10-6 15-14 15" />
    <path d="M5 19c3-4 6-6.5 9.5-8.5" />
  </Icon>
);

/** Rejoin: going round back into the game. */
export const RejoinIcon = (p: P) => (
  <Icon {...p}>
    <path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3" />
    <path d="M19.5 4.5v4.2h-4.2" />
  </Icon>
);

export const GridIcon = (p: P) => (
  <Icon {...p}>
    <rect x="4" y="4" width="7" height="7" rx="1.6" />
    <rect x="13" y="4" width="7" height="7" rx="1.6" />
    <rect x="4" y="13" width="7" height="7" rx="1.6" />
    <rect x="13" y="13" width="7" height="7" rx="1.6" />
  </Icon>
);

export const MinimizeIcon = (p: P) => (
  <Icon {...p}>
    <rect x="3.5" y="5" width="13" height="10" rx="2" />
    <path d="M8 19h12.5M20.5 9v10" />
  </Icon>
);

export const ServerIcon = (p: P) => (
  <Icon {...p}>
    <rect x="4" y="4" width="16" height="7" rx="2" />
    <rect x="4" y="13" width="16" height="7" rx="2" />
    <path d="M8 7.5h.01M8 16.5h.01" />
  </Icon>
);

export const FollowIcon = (p: P) => (
  <Icon {...p}>
    <circle cx="9" cy="8.5" r="3.5" />
    <path d="M2.5 19.5c.6-3.3 3.3-5.5 6.5-5.5 1.6 0 3 .5 4.2 1.4M15 17h6.5M18.5 14l3 3-3 3" />
  </Icon>
);

export const LockIcon = (p: P) => (
  <Icon {...p}>
    <rect x="5" y="10.5" width="14" height="9.5" rx="2" />
    <path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5" />
  </Icon>
);

export const GlobeIcon = (p: P) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M3.5 12h17M12 3.5c2.4 2.3 3.6 5.1 3.6 8.5s-1.2 6.2-3.6 8.5c-2.4-2.3-3.6-5.1-3.6-8.5S9.6 5.8 12 3.5Z" />
  </Icon>
);

export const CommandIcon = (p: P) => (
  <Icon {...p}>
    <rect x="3.5" y="5.5" width="17" height="13" rx="2.5" />
    <path d="m7.5 10 2.5 2-2.5 2M12.5 14.5h4" />
  </Icon>
);

export const ClockIcon = (p: P) => (
  <Icon {...p}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 7.5V12l3 2" />
  </Icon>
);

export const ScreenIcon = (p: P) => (
  <Icon {...p}>
    <rect x="3.5" y="4.5" width="17" height="12" rx="2" />
    <path d="M9 20h6M12 16.5V20" />
  </Icon>
);
