import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import { zh } from "./zh";

// 首发只有中文；新增语言 = 新增一个资源文件并在这里注册。
void i18n.use(initReactI18next).init({
  resources: { zh: { translation: zh } },
  lng: "zh",
  fallbackLng: "zh",
  interpolation: { escapeValue: false },
});

export default i18n;
