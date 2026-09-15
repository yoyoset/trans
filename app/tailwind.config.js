/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: ["Segoe UI", "Microsoft YaHei UI", "system-ui", "sans-serif"],
        mono: ["Cascadia Mono", "Consolas", "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};
