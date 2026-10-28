// The duck in the top bar, drawn in the app icon's colours. It sleeps under a nightcap while no
// Roblox is open and plays with a controller while any is. Phase 3 adds moods for every account
// joining and for one dropping; each mood is a class on the root, its motion in mascot.css.
export type Mood = "sleeping" | "playing";

export function Mascot({ mood, size = 52 }: { mood: Mood; size?: number }) {
  const asleep = mood === "sleeping";
  return (
    <svg className={`mascot ${mood}`} width={size} height={size} viewBox="0 0 100 100" aria-hidden>
      <ellipse className="m-shadow" cx="49" cy="91" rx="31" ry="4.2" />
      <g className="m-duck">
        <g className="m-body">
          <path className="m-tail" d="M24 57.5C16.5 56.4 10.2 51 8.4 42.6c-.3-1.5 1.2-2.5 2.5-1.8 5.6 3 12 4.6 18.4 7.4z" />
          <ellipse className="m-belly" cx="49" cy="68" rx="37" ry="20.5" />
          <path className="m-belly-shine" d="M21 61.5c4.6-8.3 15.6-12.6 27.6-12.6" />
        </g>
        <g className="m-head">
          {!asleep && <path className="m-tuft" d="M60.5 19.6c-1.4-4.6.6-8.7 4.4-9.6-1 2.6-.5 5.4 1.5 8.1z" />}
          <circle className="m-skull" cx="64" cy="37" r="18.5" />
          <path className="m-head-shine" d="M53.4 29.6c1.7-4.3 5.4-7.2 9.7-7.9" />
          <path className="m-beak-low" d="M80 41.5c5.5.4 11 .8 13.3 2.4-2.5 3-8.6 3.6-13.6 2z" />
          <path className="m-beak" d="M79.3 34.5c6.6-1.6 13.3-.6 15.5 2.5 1 1.5-.4 3.5-3.4 4.2-3.8.9-8.6.6-12.6-.6z" />
          <ellipse className="m-cheek" cx="71.5" cy="44" rx="4" ry="2.3" />
          {asleep ? (
            <path className="m-eye-shut" d="M65.5 35.5q4 3.4 8 0" />
          ) : (
            <g className="m-eye">
              <circle cx="69.5" cy="34" r="3.6" />
              <circle className="m-eye-glint" cx="70.8" cy="32.6" r="1.2" />
            </g>
          )}
          {asleep && (
            <g className="m-cap">
              <path d="M47.5 31.5C51 20 64 15.5 79 23.5c-9-12.5-29-15-37.5-8.2 2.4 4.6 4.3 9.7 6 16.2z" />
              <path className="m-cap-band" d="M46.6 30.6c9.4-6.3 21.8-8.8 33.2-6.2l-.9 3.6c-10.4-2.2-21.5-.2-30.4 5.6z" />
              <circle className="m-pompom" cx="41.5" cy="16" r="4.6" />
            </g>
          )}
        </g>
        {!asleep && (
          <g className="m-pad">
            <rect x="57" y="61" width="31" height="15" rx="7.5" />
            <path className="m-pad-dpad" d="M63.2 66.6h2v-2h2v2h2v2h-2v2h-2v-2h-2z" />
            <circle className="m-pad-a" cx="80" cy="66.4" r="1.9" />
            <circle className="m-pad-b" cx="83.6" cy="70" r="1.9" />
          </g>
        )}
        {/* Over the pad, so the wing's tip holds its left end. */}
        <path
          className="m-wing"
          d="M29.5 60.5c5-7.2 17-10.5 27.6-7.4 4.8 1.4 8.4 4.4 9.6 8.6.6 2.2-.6 3.6-2.7 3.4-3.2-.3-5.6 1.2-7.4 3.9-3.6 5.3-11.6 7-18.4 3.6-4.2-2-6.7-5.8-8.7-10.1l3.5-.4-2.5-1.6z"
        />
      </g>
      {asleep && (
        <g className="m-zs">
          <text x="80" y="22">z</text>
          <text x="87" y="13">z</text>
          <text x="93" y="5">z</text>
        </g>
      )}
    </svg>
  );
}
