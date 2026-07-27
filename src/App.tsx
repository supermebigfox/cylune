import { Mark } from "./brand/Mark";
import { t, useLocale } from "./i18n";

export function App() {
  const locale = useLocale();
  const copy = (key: string) => t(key, {}, locale);

  return (
    <main>
      <Mark label={copy("brand.mark")} size={32} />
      <p>{copy("app.localMode")}</p>
      <h1>{copy("app.name")}</h1>
    </main>
  );
}
