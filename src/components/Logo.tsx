// The app icon, the two Roblox tiles: gold in Quackblox, green in Quackblox Dev. The Dev build
// always runs from Vite's dev server (npm run app:dev), the official one from the built bundle,
// so import.meta.env.DEV tells them apart.
import gold from "../../src-tauri/icons/128x128.png";
import green from "../../src-tauri/icons-dev/128x128.png";

export const IS_DEV = import.meta.env.DEV;
export const APP_NAME = IS_DEV ? "Quackblox Dev" : "Quackblox";

export function Logo({ size }: { size: number }) {
  return <img className="logo" src={IS_DEV ? green : gold} width={size} height={size} alt="" draggable={false} />;
}
