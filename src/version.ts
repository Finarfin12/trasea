// Vite injects the release version from package.json at build time.
declare const __APP_VERSION__: string;

export const APP_VERSION: string = __APP_VERSION__;
