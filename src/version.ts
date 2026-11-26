// Vite injects the release version from package.json at build time for the desktop release.
declare const __APP_VERSION__: string;

export const APP_VERSION: string = __APP_VERSION__;
