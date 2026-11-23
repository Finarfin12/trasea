import { createContext, useContext, useMemo, type ReactNode } from "react";
import { en } from "./en";
import { id, type Dict, type Lang } from "./id";

export type { Dict, Lang };

interface I18nCtx {
	lang: Lang;
	setLang: (lang: Lang) => void;
	t: Record<keyof Dict, string>;
}

const Context = createContext<I18nCtx>({
	lang: "id",
	setLang: () => {},
	t: id,
});

export function I18nProvider({ lang, setLang, children }: { lang: Lang; setLang: (l: Lang) => void; children: ReactNode }) {
	const value = useMemo<I18nCtx>(
		() => ({
			lang,
			setLang,
			t: lang === "en" ? en : id,
		}),
		[lang, setLang],
	);
	return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useI18n(): I18nCtx {
	return useContext(Context);
}
