/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        surface: {
          DEFAULT: "#0c0d12",
          1: "#12141c",
          2: "#181b26",
          3: "#202433",
        },
        accent: {
          DEFAULT: "#6366f1",
          hover: "#818cf8",
          neon: "#d9ff38",
        },
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "Consolas", "monospace"],
      },
    },
  },
  plugins: [],
};
