import { Mark } from "./brand/Mark";
import { t } from "./i18n";

export function App() {
  return (
    <main>
      <Mark label={t("brand.mark", {}, "zh-CN")} size={32} />
      <p>本地模式</p>
      <h1>拓竹耗材管家</h1>
    </main>
  );
}
